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
        self.data_quality_score = Gauge(
            'data_ingestion_data_quality_score',
            'Data quality score (0-1)',
            ['provider']
        )
        
        # WebSocket specific metrics (Phase 4)
        self.websocket_connections = Gauge(
            'data_ingestion_websocket_connections',
            'Number of active WebSocket connections',
            ['provider', 'status']
        )
        
        self.websocket_messages = Counter(
            'data_ingestion_websocket_messages_total',
            'Total WebSocket messages received',
            ['provider', 'message_type']
        )
        
        self.websocket_reconnections = Counter(
            'data_ingestion_websocket_reconnections_total',
            'Total WebSocket reconnection attempts',
            ['provider', 'reason']
        )
        
        # Neural Prediction Metrics (Phase 6)
        self.neural_prediction_confidence = Histogram(
            'neural_trader_prediction_confidence_score',
            'Confidence scores distribution for neural predictions',
            ['model_name', 'market_regime'],
            buckets=(0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0)
        )
        
        self.neural_confidence_breakdown = Histogram(
            'neural_trader_confidence_breakdown_components',
            'Individual components of confidence score breakdown',
            ['component_type', 'model_name'],
            buckets=(-0.3, -0.2, -0.1, -0.05, 0.0, 0.05, 0.1, 0.15, 0.2, 0.3, 0.5, 1.0)
        )
        
        self.neural_prediction_accuracy = Gauge(
            'neural_trader_prediction_accuracy',
            'Current prediction accuracy for neural models',
            ['model_name', 'time_horizon', 'accuracy_type']
        )
        
        self.neural_retraining_triggers = Counter(
            'neural_trader_retraining_triggers_total',
            'Number of retraining events triggered',
            ['model_name', 'trigger_type', 'urgency_level']
        )
        
        self.neural_retraining_frequency = Histogram(
            'neural_trader_retraining_frequency_hours',
            'Time between retraining events in hours',
            ['model_name'],
            buckets=(1, 6, 12, 24, 48, 72, 168, 336, 720)  # 1h to 30 days
        )
        
        self.neural_model_ensemble_agreement = Gauge(
            'neural_trader_ensemble_agreement_score',
            'Model agreement score in ensemble predictions',
            ['ensemble_size', 'market_regime']
        )
        
        self.neural_prediction_intervals = Histogram(
            'neural_trader_prediction_intervals_width',
            'Width of prediction confidence intervals',
            ['model_name', 'volatility_regime'],
            buckets=(0.01, 0.02, 0.05, 0.1, 0.15, 0.2, 0.3, 0.5, 1.0)
        )
        
        self.neural_ensemble_weights = Gauge(
            'neural_trader_ensemble_model_weights',
            'Current dynamic weights assigned to models in ensemble',
            ['model_name', 'market_regime']
        )
        
        self.neural_prediction_latency = Histogram(
            'neural_trader_prediction_latency_seconds',
            'Time taken to generate neural predictions',
            ['model_name', 'prediction_type'],
            buckets=(0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0)
        )
        
        self.neural_model_performance_tracking = Gauge(
            'neural_trader_model_performance_metrics',
            'Performance tracking metrics for neural models',
            ['model_name', 'metric_type', 'regime']
        )
        
        self.neural_data_quality_impact = Gauge(
            'neural_trader_data_quality_confidence_impact',
            'Impact of data quality on prediction confidence',
            ['quality_component', 'severity_level']
        )
        
        self.neural_market_regime_detection = Counter(
            'neural_trader_market_regime_detection_total',
            'Market regime detection events',
            ['previous_regime', 'detected_regime', 'confidence_level']
        )
        
        self.neural_volatility_adjustments = Gauge(
            'neural_trader_volatility_based_adjustments',
            'Volatility-based confidence and weight adjustments',
            ['model_name', 'adjustment_type']
        )
        
        # Health check metrics (Phase 4)
        self.health_check_status = Gauge(
            'data_ingestion_health_status',
            'Health check status (1=healthy, 0=unhealthy)',
            ['component']
        )
        
        self.health_check_duration = Histogram(
            'data_ingestion_health_check_duration_seconds',
            'Health check duration in seconds',
            ['component']
        )
        
        # Circuit breaker metrics (Phase 4)
        self.circuit_breaker_state = Gauge(
            'data_ingestion_circuit_breaker_state',
            'Circuit breaker state (0=closed, 1=open, 2=half_open)',
            ['component']
        )
        
        self.circuit_breaker_failures = Counter(
            'data_ingestion_circuit_breaker_failures_total',
            'Total circuit breaker failures',
            ['component']
        )
        
        # File backfill metrics (Phase 4)
        self.file_backfill_progress = Gauge(
            'data_ingestion_file_backfill_progress',
            'File backfill progress (0-1)',
            ['file', 'format']
        )
        
        self.file_backfill_rows = Counter(
            'data_ingestion_file_backfill_rows_total',
            'Total rows processed in file backfill',
            ['file', 'format', 'status']
        )
        
        self.file_backfill_duration = Histogram(
            'data_ingestion_file_backfill_duration_seconds',
            'File backfill operation duration',
            ['format']
        )
        
        # Data flow metrics (Phase 4)
        self.data_flow_age = Gauge(
            'data_ingestion_data_flow_age_seconds',
            'Age of last data received in seconds',
            ['provider', 'symbol']
        )
        
        self.data_flow_rate = Gauge(
            'data_ingestion_data_flow_rate',
            'Data points per second',
            ['provider']
        )
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
        
        # Component health status (using existing health_check_status for overall)
        self.health_check_component_status = Gauge(
            'data_ingestion_health_component_status',
            'Component health status (1=healthy, 0=unhealthy)',
            ['component']
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
    
    def track_neural_prediction_operation(self, model_name: str, prediction_type: str = "standard"):
        """Decorator to track neural prediction operations with automatic metrics collection."""
        def decorator(func):
            @wraps(func)
            async def wrapper(*args, **kwargs):
                start_time = time.time()
                
                try:
                    result = await func(*args, **kwargs)
                    
                    # Track prediction latency
                    duration = time.time() - start_time
                    self.track_neural_prediction_latency(
                        model_name=model_name,
                        prediction_type=prediction_type,
                        latency_seconds=duration
                    )
                    
                    # Auto-track enhanced prediction results if the result has the right structure
                    if hasattr(result, '__iter__') and result:
                        if hasattr(result[0], 'confidence') and hasattr(result[0], 'confidence_breakdown'):
                            # This looks like a list of EnhancedPredictionResult objects
                            for prediction in result:
                                self.track_neural_prediction_with_enhanced_confidence(prediction)
                        elif hasattr(result[0], 'confidence') and hasattr(result[0], 'model_name'):
                            # This looks like a list of standard PredictionResult objects
                            for prediction in result:
                                # Extract market regime from context if available
                                market_regime = getattr(prediction, 'market_regime', 'unknown')
                                self.track_neural_prediction_confidence(
                                    model_name=prediction.model_name,
                                    market_regime=market_regime,
                                    confidence_score=prediction.confidence
                                )
                    
                    return result
                except Exception as e:
                    # Track failed predictions
                    duration = time.time() - start_time
                    self.track_neural_prediction_latency(
                        model_name=model_name,
                        prediction_type=f"{prediction_type}_failed",
                        latency_seconds=duration
                    )
                    raise
            
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
    
    # Neural Prediction Metrics Methods (Phase 6)
    
    def track_neural_prediction_confidence(self, model_name: str, market_regime: str, confidence_score: float):
        """Track confidence score distribution for neural predictions."""
        self.neural_prediction_confidence.labels(
            model_name=model_name,
            market_regime=market_regime
        ).observe(confidence_score)
    
    def track_confidence_breakdown_component(self, component_type: str, model_name: str, component_value: float):
        """Track individual components of confidence breakdown."""
        self.neural_confidence_breakdown.labels(
            component_type=component_type,
            model_name=model_name
        ).observe(component_value)
    
    def update_neural_prediction_accuracy(self, model_name: str, time_horizon: str, accuracy_type: str, accuracy_value: float):
        """Update current prediction accuracy for neural models."""
        self.neural_prediction_accuracy.labels(
            model_name=model_name,
            time_horizon=time_horizon,
            accuracy_type=accuracy_type
        ).set(accuracy_value)
    
    def track_neural_retraining_trigger(self, model_name: str, trigger_type: str, urgency_level: str):
        """Track retraining triggers and their frequency."""
        self.neural_retraining_triggers.labels(
            model_name=model_name,
            trigger_type=trigger_type,
            urgency_level=urgency_level
        ).inc()
    
    def track_neural_retraining_frequency(self, model_name: str, hours_since_last_training: float):
        """Track time between retraining events."""
        self.neural_retraining_frequency.labels(
            model_name=model_name
        ).observe(hours_since_last_training)
    
    def update_neural_ensemble_agreement(self, ensemble_size: str, market_regime: str, agreement_score: float):
        """Update model agreement score in ensemble predictions."""
        self.neural_model_ensemble_agreement.labels(
            ensemble_size=ensemble_size,
            market_regime=market_regime
        ).set(agreement_score)
    
    def track_neural_prediction_interval_width(self, model_name: str, volatility_regime: str, interval_width: float):
        """Track prediction confidence interval widths."""
        self.neural_prediction_intervals.labels(
            model_name=model_name,
            volatility_regime=volatility_regime
        ).observe(interval_width)
    
    def update_neural_ensemble_weight(self, model_name: str, market_regime: str, weight_value: float):
        """Update current dynamic weights for ensemble models."""
        self.neural_ensemble_weights.labels(
            model_name=model_name,
            market_regime=market_regime
        ).set(weight_value)
    
    def track_neural_prediction_latency(self, model_name: str, prediction_type: str, latency_seconds: float):
        """Track prediction generation latency."""
        self.neural_prediction_latency.labels(
            model_name=model_name,
            prediction_type=prediction_type
        ).observe(latency_seconds)
    
    def update_neural_model_performance(self, model_name: str, metric_type: str, regime: str, metric_value: float):
        """Update performance tracking metrics for neural models."""
        self.neural_model_performance_tracking.labels(
            model_name=model_name,
            metric_type=metric_type,
            regime=regime
        ).set(metric_value)
    
    def update_neural_data_quality_impact(self, quality_component: str, severity_level: str, impact_value: float):
        """Update data quality impact on prediction confidence."""
        self.neural_data_quality_impact.labels(
            quality_component=quality_component,
            severity_level=severity_level
        ).set(impact_value)
    
    def track_neural_market_regime_detection(self, previous_regime: str, detected_regime: str, confidence_level: str):
        """Track market regime detection events."""
        self.neural_market_regime_detection.labels(
            previous_regime=previous_regime,
            detected_regime=detected_regime,
            confidence_level=confidence_level
        ).inc()
    
    def update_neural_volatility_adjustment(self, model_name: str, adjustment_type: str, adjustment_value: float):
        """Update volatility-based adjustments."""
        self.neural_volatility_adjustments.labels(
            model_name=model_name,
            adjustment_type=adjustment_type
        ).set(adjustment_value)
    
    def track_neural_prediction_with_enhanced_confidence(self, enhanced_prediction_result):
        """
        Comprehensive tracking method for enhanced prediction results.
        This method extracts and tracks all relevant metrics from an EnhancedPredictionResult.
        """
        # Determine market regime string
        market_regime = enhanced_prediction_result.market_regime or "unknown"
        
        # Track main confidence score
        self.track_neural_prediction_confidence(
            model_name="ensemble", 
            market_regime=market_regime,
            confidence_score=enhanced_prediction_result.confidence
        )
        
        # Track confidence breakdown components
        breakdown = enhanced_prediction_result.confidence_breakdown
        component_mapping = {
            'base_confidence': breakdown.base_confidence,
            'ensemble_agreement': breakdown.ensemble_agreement,
            'historical_accuracy': breakdown.historical_accuracy,
            'market_regime_adjustment': breakdown.market_regime_adjustment,
            'volatility_penalty': breakdown.volatility_penalty,
            'temporal_distance_penalty': breakdown.temporal_distance_penalty
        }
        
        for component_type, value in component_mapping.items():
            self.track_confidence_breakdown_component(
                component_type=component_type,
                model_name="ensemble",
                component_value=value
            )
        
        # Track data quality factor
        self.update_neural_data_quality_impact(
            quality_component="overall_quality_factor",
            severity_level="current",
            impact_value=breakdown.data_quality_factor
        )
        
        # Track model agreement
        ensemble_size_str = str(enhanced_prediction_result.ensemble_size)
        self.update_neural_ensemble_agreement(
            ensemble_size=ensemble_size_str,
            market_regime=market_regime,
            agreement_score=enhanced_prediction_result.model_agreement_score
        )
        
        # Track prediction interval width
        interval_width = (enhanced_prediction_result.interval_high - enhanced_prediction_result.interval_low) / enhanced_prediction_result.value
        volatility_regime = "high" if enhanced_prediction_result.volatility_adjustment > 1.2 else "normal" if enhanced_prediction_result.volatility_adjustment > 0.8 else "low"
        
        self.track_neural_prediction_interval_width(
            model_name="ensemble",
            volatility_regime=volatility_regime,
            interval_width=interval_width
        )
        
        # Track volatility adjustments
        self.update_neural_volatility_adjustment(
            model_name="ensemble",
            adjustment_type="interval_multiplier",
            adjustment_value=enhanced_prediction_result.volatility_adjustment
        )
    
    def track_neural_retraining_decision(self, retraining_metrics):
        """
        Track retraining decision metrics from RetrainingMetrics object.
        """
        # Determine urgency level
        urgency_level = "critical" if retraining_metrics.urgency_score > 3.0 else "high" if retraining_metrics.urgency_score > 1.5 else "medium" if retraining_metrics.urgency_score > 0.5 else "low"
        
        # Track retraining trigger if needed
        if retraining_metrics.should_retrain:
            self.track_neural_retraining_trigger(
                model_name="ensemble",
                trigger_type=retraining_metrics.primary_trigger,
                urgency_level=urgency_level
            )
            
            # Track frequency since last training
            self.track_neural_retraining_frequency(
                model_name="ensemble",
                hours_since_last_training=float(retraining_metrics.hours_since_training)
            )
        
        # Update current accuracy metric
        self.update_neural_prediction_accuracy(
            model_name="ensemble",
            time_horizon="recent",
            accuracy_type="exponential_weighted",
            accuracy_value=retraining_metrics.current_accuracy
        )
    
    def track_neural_ensemble_performance(self, ensemble_stats):
        """
        Track comprehensive ensemble performance statistics.
        """
        current_regime = ensemble_stats.get("current_regime", "unknown")
        
        # Track dynamic weights
        if "dynamic_weights" in ensemble_stats:
            weights = ensemble_stats["dynamic_weights"]
            for model_name, weight in weights.items():
                self.update_neural_ensemble_weight(
                    model_name=model_name,
                    market_regime=current_regime,
                    weight_value=weight
                )
        
        # Track model performances
        if "model_performances" in ensemble_stats:
            performances = ensemble_stats["model_performances"]
            for model_name, performance in performances.items():
                if isinstance(performance, dict):
                    # Track recent accuracy
                    if "recent_accuracy" in performance:
                        self.update_neural_model_performance(
                            model_name=model_name,
                            metric_type="recent_accuracy",
                            regime=current_regime,
                            metric_value=performance["recent_accuracy"]
                        )
                    
                    # Track confidence score
                    if "confidence_score" in performance:
                        self.update_neural_model_performance(
                            model_name=model_name,
                            metric_type="confidence_calibration",
                            regime=current_regime,
                            metric_value=performance["confidence_score"]
                        )
                    
                    # Track stability score
                    if "stability_score" in performance:
                        self.update_neural_model_performance(
                            model_name=model_name,
                            metric_type="stability",
                            regime=current_regime,
                            metric_value=performance["stability_score"]
                        )
        
        # Track volatility adjustments
        if "volatility_adjustments" in ensemble_stats:
            adjustments = ensemble_stats["volatility_adjustments"]
            for model_name, adjustment in adjustments.items():
                self.update_neural_volatility_adjustment(
                    model_name=model_name,
                    adjustment_type="ensemble_weight_adjustment",
                    adjustment_value=adjustment
                )


# Global metrics instance
metrics = Metrics()


def start_metrics_server(port: int = 9090):
    """Start Prometheus metrics server."""
    from prometheus_client import start_http_server
    start_http_server(port)
    return port