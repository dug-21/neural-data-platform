"""Helper utilities for async metric collection."""
import asyncio
import time
from typing import Optional, Dict, Any, List, Callable
from functools import wraps
from contextlib import asynccontextmanager
import inspect

from .metrics import metrics
from .logging import get_logger

logger = get_logger(__name__)


class MetricCollector:
    """Async-friendly metric collection helper."""
    
    def __init__(self):
        self.batch_buffer: Dict[str, List[float]] = {}
        self.flush_interval = 10  # seconds
        self._flush_task: Optional[asyncio.Task] = None
        self._running = False
    
    async def start(self):
        """Start background metric flushing."""
        self._running = True
        self._flush_task = asyncio.create_task(self._flush_loop())
    
    async def stop(self):
        """Stop background metric flushing."""
        self._running = False
        if self._flush_task:
            self._flush_task.cancel()
            try:
                await self._flush_task
            except asyncio.CancelledError:
                pass
        
        # Final flush
        await self._flush_metrics()
    
    async def _flush_loop(self):
        """Background task to periodically flush metrics."""
        while self._running:
            try:
                await asyncio.sleep(self.flush_interval)
                await self._flush_metrics()
            except Exception as e:
                logger.error(f"Error flushing metrics: {e}")
    
    async def _flush_metrics(self):
        """Flush buffered metrics."""
        for key, values in self.batch_buffer.items():
            if values:
                # Calculate aggregates
                avg_value = sum(values) / len(values)
                min_value = min(values)
                max_value = max(values)
                
                # Log aggregated metrics
                logger.debug(
                    f"Metric {key}: avg={avg_value:.3f}, "
                    f"min={min_value:.3f}, max={max_value:.3f}, "
                    f"count={len(values)}"
                )
                
                # Clear buffer
                values.clear()
    
    def buffer_metric(self, key: str, value: float):
        """Buffer a metric value for batch processing."""
        if key not in self.batch_buffer:
            self.batch_buffer[key] = []
        self.batch_buffer[key].append(value)


# Global collector instance
collector = MetricCollector()


def track_async_batch(metric_name: str, batch_size: int):
    """Decorator for tracking async batch operations."""
    def decorator(func):
        @wraps(func)
        async def wrapper(*args, **kwargs):
            start_time = time.time()
            
            try:
                result = await func(*args, **kwargs)
                
                # Track batch metrics
                duration = time.time() - start_time
                if batch_size > 0:
                    throughput = batch_size / duration
                    collector.buffer_metric(f"{metric_name}_throughput", throughput)
                
                return result
            except Exception as e:
                metrics.processing_errors.labels(
                    provider="batch",
                    error_type=type(e).__name__
                ).inc()
                raise
        
        return wrapper
    return decorator


@asynccontextmanager
async def track_backpressure(pipeline: str, stage: str, queue_size: int, max_size: int):
    """Context manager to track pipeline backpressure."""
    try:
        # Calculate backpressure ratio
        pressure = queue_size / max_size if max_size > 0 else 0
        metrics.update_pipeline_backpressure(pipeline, stage, pressure)
        
        yield
    finally:
        # Reset backpressure after processing
        metrics.update_pipeline_backpressure(pipeline, stage, 0)


class ProviderHealthTracker:
    """Track provider health metrics."""
    
    def __init__(self):
        self.error_counts: Dict[str, int] = {}
        self.success_counts: Dict[str, int] = {}
        self.response_times: Dict[str, List[float]] = {}
        self.last_update: Dict[str, float] = {}
    
    def record_success(self, provider: str, response_time: float):
        """Record a successful operation."""
        if provider not in self.success_counts:
            self.success_counts[provider] = 0
            self.response_times[provider] = []
        
        self.success_counts[provider] += 1
        self.response_times[provider].append(response_time)
        
        # Keep only last 100 response times
        if len(self.response_times[provider]) > 100:
            self.response_times[provider] = self.response_times[provider][-100:]
        
        self.last_update[provider] = time.time()
        self._update_health_score(provider)
    
    def record_error(self, provider: str):
        """Record an error."""
        if provider not in self.error_counts:
            self.error_counts[provider] = 0
        
        self.error_counts[provider] += 1
        self.last_update[provider] = time.time()
        self._update_health_score(provider)
    
    def _update_health_score(self, provider: str):
        """Calculate and update provider health score."""
        success = self.success_counts.get(provider, 0)
        errors = self.error_counts.get(provider, 0)
        total = success + errors
        
        if total == 0:
            health_score = 1.0
        else:
            # Base score on success rate
            health_score = success / total
            
            # Factor in response time if available
            if provider in self.response_times and self.response_times[provider]:
                avg_response = sum(self.response_times[provider]) / len(self.response_times[provider])
                # Penalize if avg response > 1 second
                if avg_response > 1.0:
                    health_score *= (1.0 / avg_response)
        
        # Update metric
        metrics.update_provider_health(provider, health_score)
        
        # Reset counters periodically (every 1000 operations)
        if total >= 1000:
            self.success_counts[provider] = self.success_counts[provider] // 2
            self.error_counts[provider] = self.error_counts[provider] // 2


# Global health tracker
health_tracker = ProviderHealthTracker()


def track_data_quality(provider: str):
    """Decorator to track data quality metrics."""
    def decorator(func):
        @wraps(func)
        async def wrapper(*args, **kwargs):
            result = await func(*args, **kwargs)
            
            if isinstance(result, (list, tuple)) and len(result) == 2:
                valid_data, invalid_data = result
                
                total = len(valid_data) + len(invalid_data)
                if total > 0:
                    quality_score = len(valid_data) / total
                    metrics.update_provider_data_quality(
                        provider, 
                        "validation_rate", 
                        quality_score
                    )
                
                # Track specific quality issues
                if invalid_data:
                    for item in invalid_data[:10]:  # Sample first 10
                        if 'reason' in item:
                            metrics.data_quality_issues.labels(
                                issue_type=item['reason']
                            ).inc()
            
            return result
        
        return wrapper
    return decorator


# Connection pool monitoring
async def monitor_connection_pools():
    """Monitor and update connection pool metrics."""
    while True:
        try:
            # This would be called by the actual connection pool managers
            # Example implementation shown
            await asyncio.sleep(30)  # Check every 30 seconds
        except asyncio.CancelledError:
            break
        except Exception as e:
            logger.error(f"Error monitoring connection pools: {e}")


# Scheduler lag tracking
def calculate_scheduler_lag(scheduled_time: float, actual_time: float) -> float:
    """Calculate scheduler execution lag."""
    return max(0, actual_time - scheduled_time)


# Async task monitoring
class AsyncTaskMonitor:
    """Monitor async task execution."""
    
    def __init__(self):
        self.active_tasks: Dict[str, int] = {}
        self.task_queues: Dict[str, int] = {}
    
    def task_started(self, task_type: str):
        """Record task start."""
        if task_type not in self.active_tasks:
            self.active_tasks[task_type] = 0
        self.active_tasks[task_type] += 1
        metrics.concurrent_tasks.labels(task_type=task_type).set(self.active_tasks[task_type])
    
    def task_completed(self, task_type: str):
        """Record task completion."""
        if task_type in self.active_tasks:
            self.active_tasks[task_type] = max(0, self.active_tasks[task_type] - 1)
            metrics.concurrent_tasks.labels(task_type=task_type).set(self.active_tasks[task_type])
    
    def update_queue_size(self, task_type: str, size: int):
        """Update task queue size."""
        self.task_queues[task_type] = size
        metrics.async_task_queue_size.labels(task_type=task_type).set(size)


# Global task monitor
task_monitor = AsyncTaskMonitor()