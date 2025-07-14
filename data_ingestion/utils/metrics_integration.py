"""Clean metrics integration for data ingestion service."""
import asyncio
from typing import Optional, Dict, Any, List
from contextlib import asynccontextmanager
from .metrics import metrics
from .logging import get_logger

logger = get_logger(__name__)


class MetricsCollector:
    """Clean metrics collection without workarounds."""
    
    def __init__(self):
        self.metrics = metrics
        self._provider_health_scores = {}
        self._data_quality_scores = {}
        self._active_streams = 0
        self._active_connections = {}
    
    def start_collection(self):
        """Start background metrics collection."""
        logger.info("Starting clean metrics collection")
        # Start periodic health checks
        asyncio.create_task(self._periodic_health_checks())
    
    async def _periodic_health_checks(self):
        """Periodic health checks and metrics updates."""
        while True:
            try:
                # Update system metrics
                self._update_system_metrics()
                
                # Check provider health
                await self._check_provider_health()
                
                # Update data quality metrics
                await self._update_data_quality_metrics()
                
                # Sleep for 30 seconds
                await asyncio.sleep(30)
                
            except Exception as e:
                logger.error(f"Error in periodic health checks: {e}")
                await asyncio.sleep(60)  # Wait longer on error
    
    def _update_system_metrics(self):
        """Update system-level metrics."""
        # Update active streams
        self.metrics.active_streams.set(self._active_streams)
        
        # Update active connections
        for conn_type, count in self._active_connections.items():
            self.metrics.active_connections.labels(connection_type=conn_type).set(count)
    
    async def _check_provider_health(self):
        """Check and update provider health scores."""
        for provider_name, health_score in self._provider_health_scores.items():
            self.metrics.update_provider_health(provider_name, health_score)
    
    async def _update_data_quality_metrics(self):
        """Update data quality metrics."""
        for provider_name, quality_scores in self._data_quality_scores.items():
            for metric_type, score in quality_scores.items():
                self.metrics.update_provider_data_quality(provider_name, metric_type, score)
    
    # Provider tracking methods
    def track_provider_connection(self, provider_name: str, connected: bool):
        """Track provider connection status."""
        if connected:
            self._active_connections[provider_name] = 1
            self._provider_health_scores[provider_name] = 1.0
        else:
            self._active_connections[provider_name] = 0
            self._provider_health_scores[provider_name] = 0.0
    
    def track_stream_start(self, stream_id: str):
        """Track stream start."""
        self._active_streams += 1
        logger.info(f"Stream {stream_id} started, active streams: {self._active_streams}")
    
    def track_stream_stop(self, stream_id: str):
        """Track stream stop."""
        self._active_streams = max(0, self._active_streams - 1)
        logger.info(f"Stream {stream_id} stopped, active streams: {self._active_streams}")
    
    def track_data_quality(self, provider_name: str, metric_type: str, score: float):
        """Track data quality scores."""
        if provider_name not in self._data_quality_scores:
            self._data_quality_scores[provider_name] = {}
        
        self._data_quality_scores[provider_name][metric_type] = score
    
    def track_provider_error(self, provider_name: str, error_type: str):
        """Track provider errors."""
        self.metrics.processing_errors.labels(provider=provider_name, error_type=error_type).inc()
        
        # Reduce health score on errors
        current_health = self._provider_health_scores.get(provider_name, 1.0)
        new_health = max(0.0, current_health - 0.1)  # Reduce by 10%
        self._provider_health_scores[provider_name] = new_health
    
    def track_data_processed(self, provider_name: str, data_type: str, count: int = 1):
        """Track data points processed."""
        self.metrics.data_points_processed.labels(provider=provider_name, data_type=data_type).inc(count)
    
    def track_rate_limit_hit(self, provider_name: str):
        """Track rate limit hits."""
        self.metrics.rate_limit_hits.labels(provider=provider_name).inc()
    
    def track_validation_failure(self, provider_name: str):
        """Track validation failures."""
        self.metrics.validation_failures.labels(provider=provider_name).inc()
    
    def track_streaming_error(self, provider_name: str):
        """Track streaming errors."""
        self.metrics.streaming_errors.labels(provider=provider_name).inc()
    
    # Context managers for automatic tracking
    @asynccontextmanager
    async def track_api_call(self, provider_name: str, endpoint: str):
        """Context manager for API call tracking."""
        import time
        start_time = time.time()
        status = "success"
        
        try:
            yield
        except Exception as e:
            status = "error"
            self.track_provider_error(provider_name, type(e).__name__)
            raise
        finally:
            duration = time.time() - start_time
            self.metrics.api_requests_total.labels(
                provider=provider_name,
                endpoint=endpoint,
                status=status
            ).inc()
            self.metrics.api_request_duration.labels(
                provider=provider_name,
                endpoint=endpoint
            ).observe(duration)
    
    @asynccontextmanager
    async def track_storage_operation(self, storage_type: str, operation: str):
        """Context manager for storage operation tracking."""
        import time
        start_time = time.time()
        status = "success"
        
        try:
            yield
        except Exception as e:
            status = "error"
            raise
        finally:
            duration = time.time() - start_time
            self.metrics.storage_operations.labels(
                storage_type=storage_type,
                operation=operation,
                status=status
            ).inc()
            self.metrics.storage_duration.labels(
                storage_type=storage_type,
                operation=operation
            ).observe(duration)
    
    @asynccontextmanager
    async def track_redis_publish(self, channel_type: str, message_size: int = 0):
        """Context manager for Redis publish tracking."""
        import time
        start_time = time.time()
        status = "success"
        
        try:
            yield
        except Exception as e:
            status = "error"
            raise
        finally:
            duration = time.time() - start_time
            self.metrics.redis_publish_total.labels(
                channel_type=channel_type,
                status=status
            ).inc()
            self.metrics.redis_publish_duration.labels(
                channel_type=channel_type
            ).observe(duration)
            
            if message_size > 0:
                self.metrics.redis_publish_size.labels(
                    channel_type=channel_type
                ).observe(message_size)


# Global metrics collector instance
metrics_collector = MetricsCollector()