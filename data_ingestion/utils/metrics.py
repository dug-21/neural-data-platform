"""Prometheus metrics for monitoring."""
from prometheus_client import Counter, Histogram, Gauge, Summary, Info
from functools import wraps
import time
import asyncio
from typing import Optional, Callable, Any, Dict
from contextlib import asynccontextmanager
import inspect


class Metrics:
    """Application metrics for monitoring."""
    
    def __init__(self):
        # Request metrics
        self.api_requests_total = Counter(
            'data_ingestion_api_requests_total',
            'Total number of API requests',
            ['provider', 'endpoint', 'status']
        )
        
        self.api_request_duration = Histogram(
            'data_ingestion_api_request_duration_seconds',
            'API request duration in seconds',
            ['provider', 'endpoint']
        )
        
        # Data processing metrics
        self.data_points_processed = Counter(
            'data_ingestion_points_processed_total',
            'Total number of data points processed',
            ['provider', 'data_type']
        )
        
        self.processing_errors = Counter(
            'data_ingestion_processing_errors_total',
            'Total number of processing errors',
            ['provider', 'error_type']
        )
        
        # Storage metrics
        self.storage_operations = Counter(
            'data_ingestion_storage_operations_total',
            'Total number of storage operations',
            ['storage_type', 'operation', 'status']
        )
        
        self.storage_duration = Histogram(
            'data_ingestion_storage_duration_seconds',
            'Storage operation duration in seconds',
            ['storage_type', 'operation']
        )
        
        # System metrics
        self.active_connections = Gauge(
            'data_ingestion_active_connections',
            'Number of active connections',
            ['connection_type']
        )
        
        self.queue_size = Gauge(
            'data_ingestion_queue_size',
            'Current queue size',
            ['queue_name']
        )
        
        # Rate limiting metrics
        self.rate_limit_hits = Counter(
            'data_ingestion_rate_limit_hits_total',
            'Number of rate limit hits',
            ['provider']
        )
        
        # Streaming metrics
        self.streaming_errors = Counter(
            'data_ingestion_streaming_errors_total',
            'Number of streaming errors',
            ['provider']
        )
        
        self.active_streams = Gauge(
            'data_ingestion_active_streams',
            'Number of active data streams'
        )
        
        # Validation metrics
        self.validation_failures = Counter(
            'data_ingestion_validation_failures_total',
            'Number of validation failures',
            ['provider']
        )
        
        # Additional processing metrics by stage
        self.processing_errors_by_stage = Counter(
            'data_ingestion_processing_errors_by_stage_total',
            'Total number of processing errors by stage',
            ['stage']
        )
        
        # Data quality metrics
        self.data_quality_issues = Counter(
            'data_ingestion_data_quality_issues_total',
            'Number of data quality issues',
            ['issue_type']
        )
        
        # Batch job metrics
        self.batch_job_duration = Histogram(
            'data_ingestion_batch_job_duration_seconds',
            'Duration of batch jobs',
            ['job_id']
        )
        
        self.batch_job_errors = Counter(
            'data_ingestion_batch_job_errors_total',
            'Number of batch job errors',
            ['job_id', 'provider']
        )
        
        self.batch_job_success = Counter(
            'data_ingestion_batch_job_success_total',
            'Number of successful batch jobs',
            ['job_id']
        )
        
        # Pipeline metrics
        self.pipeline_stage_duration = Histogram(
            'data_ingestion_pipeline_stage_duration_seconds',
            'Duration of pipeline stages',
            ['pipeline', 'stage'],
            buckets=(0.001, 0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 7.5, 10.0)
        )
        
        self.pipeline_throughput = Summary(
            'data_ingestion_pipeline_throughput_items_per_second',
            'Pipeline throughput in items per second',
            ['pipeline']
        )
        
        self.pipeline_backpressure = Gauge(
            'data_ingestion_pipeline_backpressure',
            'Pipeline backpressure indicator',
            ['pipeline', 'stage']
        )
        
        # Provider-level metrics
        self.provider_health_score = Gauge(
            'data_ingestion_provider_health_score',
            'Provider health score (0-1)',
            ['provider']
        )
        
        self.provider_latency = Summary(
            'data_ingestion_provider_latency_seconds',
            'Provider response latency',
            ['provider', 'operation']
        )
        
        self.provider_data_quality = Gauge(
            'data_ingestion_provider_data_quality_score',
            'Provider data quality score (0-1)',
            ['provider', 'metric_type']
        )
        
        # Redis publish metrics
        self.redis_publish_total = Counter(
            'data_ingestion_redis_publish_total',
            'Total Redis publish operations',
            ['channel_type', 'status']
        )
        
        self.redis_publish_duration = Histogram(
            'data_ingestion_redis_publish_duration_seconds',
            'Redis publish operation duration',
            ['channel_type']
        )
        
        self.redis_publish_size = Histogram(
            'data_ingestion_redis_publish_size_bytes',
            'Size of Redis published messages',
            ['channel_type'],
            buckets=(100, 500, 1000, 5000, 10000, 50000, 100000)
        )
        
        # Database write metrics
        self.db_write_batch_size = Histogram(
            'data_ingestion_db_write_batch_size',
            'Database write batch size',
            ['table', 'operation'],
            buckets=(1, 10, 50, 100, 500, 1000, 5000, 10000)
        )
        
        self.db_write_duration = Histogram(
            'data_ingestion_db_write_duration_seconds',
            'Database write operation duration',
            ['table', 'operation'],
            buckets=(0.001, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0)
        )
        
        self.db_write_throughput = Summary(
            'data_ingestion_db_write_throughput_rows_per_second',
            'Database write throughput',
            ['table']
        )
        
        self.db_connection_pool_size = Gauge(
            'data_ingestion_db_connection_pool_size',
            'Database connection pool size',
            ['pool_name', 'state']
        )
        
        # Health check metrics
        self.health_check_status = Gauge(
            'data_ingestion_health_status',
            'Overall health status (1=healthy, 0=unhealthy)'
        )
        
        self.health_check_component_status = Gauge(
            'data_ingestion_health_component_status',
            'Component health status (1=healthy, 0=unhealthy)',
            ['component']
        )
        
        self.data_flow_age_seconds = Gauge(
            'data_ingestion_data_flow_age_seconds',
            'Age of last received data in seconds',
            ['provider', 'symbol']
        )
        
        # Async operation metrics
        self.async_task_duration = Histogram(
            'data_ingestion_async_task_duration_seconds',
            'Async task duration',
            ['task_type'],
            buckets=(0.001, 0.01, 0.1, 0.5, 1.0, 5.0, 10.0, 30.0, 60.0)
        )
        
        self.async_task_queue_size = Gauge(
            'data_ingestion_async_task_queue_size',
            'Async task queue size',
            ['task_type']
        )
        
        self.concurrent_tasks = Gauge(
            'data_ingestion_concurrent_tasks',
            'Number of concurrent async tasks',
            ['task_type']
        )
        
        # Scheduler metrics
        self.scheduler_run_duration = Histogram(
            'data_ingestion_scheduler_run_duration_seconds',
            'Scheduler run duration',
            ['scheduler_type', 'job_name']
        )
        
        self.scheduler_lag = Gauge(
            'data_ingestion_scheduler_lag_seconds',
            'Scheduler execution lag',
            ['scheduler_type', 'job_name']
        )
        
        self.scheduler_failures = Counter(
            'data_ingestion_scheduler_failures_total',
            'Scheduler job failures',
            ['scheduler_type', 'job_name', 'failure_reason']
        )
        
        # System info
        self.system_info = Info(
            'data_ingestion_system',
            'System information'
        )
        self.system_info.info({
            'version': '1.0.0',
            'environment': 'production'
        })
    
    def track_api_request(self, provider: str, endpoint: str):
        """Decorator to track API request metrics."""
        def decorator(func):
            if asyncio.iscoroutinefunction(func):
                @wraps(func)
                async def async_wrapper(*args, **kwargs):
                    start_time = time.time()
                    status = "success"
                    
                    try:
                        result = await func(*args, **kwargs)
                        return result
                    except Exception as e:
                        status = "error"
                        raise
                    finally:
                        duration = time.time() - start_time
                        self.api_requests_total.labels(
                            provider=provider,
                            endpoint=endpoint,
                            status=status
                        ).inc()
                        self.api_request_duration.labels(
                            provider=provider,
                            endpoint=endpoint
                        ).observe(duration)
                        self.provider_latency.labels(
                            provider=provider,
                            operation=endpoint
                        ).observe(duration)
                
                return async_wrapper
            else:
                @wraps(func)
                def sync_wrapper(*args, **kwargs):
                    start_time = time.time()
                    status = "success"
                    
                    try:
                        result = func(*args, **kwargs)
                        return result
                    except Exception as e:
                        status = "error"
                        raise
                    finally:
                        duration = time.time() - start_time
                        self.api_requests_total.labels(
                            provider=provider,
                            endpoint=endpoint,
                            status=status
                        ).inc()
                        self.api_request_duration.labels(
                            provider=provider,
                            endpoint=endpoint
                        ).observe(duration)
                        self.provider_latency.labels(
                            provider=provider,
                            operation=endpoint
                        ).observe(duration)
                
                return sync_wrapper
        return decorator
    
    def track_storage_operation(self, storage_type: str, operation: str):
        """Decorator to track storage operation metrics."""
        def decorator(func):
            @wraps(func)
            async def wrapper(*args, **kwargs):
                start_time = time.time()
                status = "success"
                
                try:
                    result = await func(*args, **kwargs)
                    return result
                except Exception as e:
                    status = "error"
                    raise
                finally:
                    duration = time.time() - start_time
                    self.storage_operations.labels(
                        storage_type=storage_type,
                        operation=operation,
                        status=status
                    ).inc()
                    self.storage_duration.labels(
                        storage_type=storage_type,
                        operation=operation
                    ).observe(duration)
            
            return wrapper
        return decorator
    
    def track_pipeline_stage(self, pipeline: str, stage: str):
        """Decorator to track pipeline stage execution."""
        def decorator(func):
            @wraps(func)
            async def wrapper(*args, **kwargs):
                start_time = time.time()
                
                # Track concurrent execution
                self.concurrent_tasks.labels(task_type=f"{pipeline}_{stage}").inc()
                
                try:
                    result = await func(*args, **kwargs)
                    return result
                finally:
                    duration = time.time() - start_time
                    self.pipeline_stage_duration.labels(
                        pipeline=pipeline,
                        stage=stage
                    ).observe(duration)
                    self.concurrent_tasks.labels(task_type=f"{pipeline}_{stage}").dec()
            
            return wrapper
        return decorator
    
    def track_async_task(self, task_type: str):
        """Decorator to track async task execution."""
        def decorator(func):
            @wraps(func)
            async def wrapper(*args, **kwargs):
                start_time = time.time()
                
                # Track concurrent tasks
                self.concurrent_tasks.labels(task_type=task_type).inc()
                
                try:
                    result = await func(*args, **kwargs)
                    return result
                finally:
                    duration = time.time() - start_time
                    self.async_task_duration.labels(
                        task_type=task_type
                    ).observe(duration)
                    self.concurrent_tasks.labels(task_type=task_type).dec()
            
            return wrapper
        return decorator
    
    def track_db_write(self, table: str, operation: str = "insert"):
        """Decorator to track database write operations."""
        def decorator(func):
            @wraps(func)
            async def wrapper(*args, **kwargs):
                start_time = time.time()
                batch_size = 0
                
                # Try to extract batch size from arguments
                if args and hasattr(args[0], '__len__'):
                    batch_size = len(args[0])
                elif 'data' in kwargs and hasattr(kwargs['data'], '__len__'):
                    batch_size = len(kwargs['data'])
                elif 'records' in kwargs and hasattr(kwargs['records'], '__len__'):
                    batch_size = len(kwargs['records'])
                
                try:
                    result = await func(*args, **kwargs)
                    
                    # Track batch size
                    if batch_size > 0:
                        self.db_write_batch_size.labels(
                            table=table,
                            operation=operation
                        ).observe(batch_size)
                    
                    return result
                finally:
                    duration = time.time() - start_time
                    self.db_write_duration.labels(
                        table=table,
                        operation=operation
                    ).observe(duration)
                    
                    # Track throughput
                    if batch_size > 0 and duration > 0:
                        throughput = batch_size / duration
                        self.db_write_throughput.labels(
                            table=table
                        ).observe(throughput)
            
            return wrapper
        return decorator
    
    def track_redis_publish(self, channel_type: str):
        """Decorator to track Redis publish operations."""
        def decorator(func):
            @wraps(func)
            async def wrapper(*args, **kwargs):
                start_time = time.time()
                status = "success"
                message_size = 0
                
                # Try to extract message size
                if len(args) > 1 and isinstance(args[1], (str, bytes)):
                    message_size = len(args[1])
                elif 'message' in kwargs:
                    message = kwargs['message']
                    if isinstance(message, (str, bytes)):
                        message_size = len(message)
                
                try:
                    result = await func(*args, **kwargs)
                    return result
                except Exception as e:
                    status = "error"
                    raise
                finally:
                    duration = time.time() - start_time
                    
                    self.redis_publish_total.labels(
                        channel_type=channel_type,
                        status=status
                    ).inc()
                    
                    self.redis_publish_duration.labels(
                        channel_type=channel_type
                    ).observe(duration)
                    
                    if message_size > 0:
                        self.redis_publish_size.labels(
                            channel_type=channel_type
                        ).observe(message_size)
            
            return wrapper
        return decorator
    
    def track_scheduler_job(self, scheduler_type: str, job_name: str):
        """Decorator to track scheduler job execution."""
        def decorator(func):
            @wraps(func)
            async def wrapper(*args, **kwargs):
                start_time = time.time()
                
                try:
                    result = await func(*args, **kwargs)
                    return result
                except Exception as e:
                    self.scheduler_failures.labels(
                        scheduler_type=scheduler_type,
                        job_name=job_name,
                        failure_reason=type(e).__name__
                    ).inc()
                    raise
                finally:
                    duration = time.time() - start_time
                    self.scheduler_run_duration.labels(
                        scheduler_type=scheduler_type,
                        job_name=job_name
                    ).observe(duration)
            
            return wrapper
        return decorator
    
    @asynccontextmanager
    async def track_pipeline_throughput(self, pipeline: str, item_count: int):
        """Context manager to track pipeline throughput."""
        start_time = time.time()
        try:
            yield
        finally:
            duration = time.time() - start_time
            if duration > 0:
                throughput = item_count / duration
                self.pipeline_throughput.labels(pipeline=pipeline).observe(throughput)
    
    def update_provider_health(self, provider: str, health_score: float):
        """Update provider health score (0-1)."""
        self.provider_health_score.labels(provider=provider).set(health_score)
    
    def update_provider_data_quality(self, provider: str, metric_type: str, score: float):
        """Update provider data quality score (0-1)."""
        self.provider_data_quality.labels(
            provider=provider,
            metric_type=metric_type
        ).set(score)
    
    def update_pipeline_backpressure(self, pipeline: str, stage: str, pressure: float):
        """Update pipeline backpressure indicator (0-1)."""
        self.pipeline_backpressure.labels(
            pipeline=pipeline,
            stage=stage
        ).set(pressure)
    
    def update_scheduler_lag(self, scheduler_type: str, job_name: str, lag_seconds: float):
        """Update scheduler execution lag."""
        self.scheduler_lag.labels(
            scheduler_type=scheduler_type,
            job_name=job_name
        ).set(lag_seconds)
    
    def update_db_connection_pool(self, pool_name: str, active: int, idle: int, total: int):
        """Update database connection pool metrics."""
        self.db_connection_pool_size.labels(
            pool_name=pool_name,
            state='active'
        ).set(active)
        self.db_connection_pool_size.labels(
            pool_name=pool_name,
            state='idle'
        ).set(idle)
        self.db_connection_pool_size.labels(
            pool_name=pool_name,
            state='total'
        ).set(total)


# Global metrics instance
metrics = Metrics()


def start_metrics_server(port: int = 9090):
    """Start Prometheus metrics server."""
    from prometheus_client import start_http_server
    start_http_server(port)
    return port