"""Prometheus metrics for monitoring."""
from prometheus_client import Counter, Histogram, Gauge, Summary
from functools import wraps
import time


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
    
    def track_api_request(self, provider: str, endpoint: str):
        """Decorator to track API request metrics."""
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
                    self.api_requests_total.labels(
                        provider=provider,
                        endpoint=endpoint,
                        status=status
                    ).inc()
                    self.api_request_duration.labels(
                        provider=provider,
                        endpoint=endpoint
                    ).observe(duration)
            
            return wrapper
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


# Global metrics instance
metrics = Metrics()


def start_metrics_server(port: int = 9090):
    """Start Prometheus metrics server."""
    from prometheus_client import start_http_server
    start_http_server(port)
    return port