"""Enhanced metrics collection for comprehensive observability."""
from prometheus_client import Counter, Histogram, Gauge, Summary, Info
from contextlib import asynccontextmanager
import time
import asyncio
from typing import Optional, Dict, Any, Callable
from functools import wraps
from datetime import datetime
import psutil
import gc

from .logging import get_logger

logger = get_logger(__name__)


class EnhancedMetrics:
    """Enhanced metrics collection with WebSocket and system monitoring."""
    
    def __init__(self):
        """Initialize all metric categories."""
        self._init_websocket_metrics()
        self._init_data_flow_metrics()
        self._init_storage_metrics()
        self._init_system_metrics()
        self._init_provider_metrics()
        
        # Track last update times for freshness monitoring
        self._last_update_times: Dict[str, float] = {}
        self._freshness_monitor_task = None
        
    def _init_websocket_metrics(self):
        """Initialize WebSocket-specific metrics."""
        # Connection state tracking
        self.websocket_connection_state = Gauge(
            'data_ingestion_websocket_connection_state',
            'WebSocket connection state (0=disconnected, 1=connecting, 2=authenticating, 3=connected, 4=reconnecting, 5=failed)',
            ['provider', 'endpoint']
        )
        
        # Connection lifecycle
        self.websocket_connection_duration = Histogram(
            'data_ingestion_websocket_connection_duration_seconds',
            'Duration of WebSocket connections',
            ['provider', 'endpoint', 'termination_reason'],
            buckets=(60, 300, 900, 1800, 3600, 7200, 14400, 28800, 86400)
        )
        
        # Reconnection metrics
        self.websocket_reconnection_attempts = Counter(
            'data_ingestion_websocket_reconnection_attempts_total',
            'Total number of reconnection attempts',
            ['provider', 'endpoint']
        )
        
        self.websocket_reconnection_success = Counter(
            'data_ingestion_websocket_reconnection_success_total',
            'Successful reconnection attempts',
            ['provider', 'endpoint']
        )
        
        # Message flow
        self.websocket_messages_received = Counter(
            'data_ingestion_websocket_messages_received_total',
            'Total messages received via WebSocket',
            ['provider', 'message_type']
        )
        
        self.websocket_messages_processed = Counter(
            'data_ingestion_websocket_messages_processed_total',
            'Total messages successfully processed',
            ['provider', 'message_type', 'status']
        )
        
        # Buffer metrics
        self.websocket_buffer_size = Gauge(
            'data_ingestion_websocket_buffer_size',
            'Current WebSocket message buffer size',
            ['provider', 'buffer_type']
        )
        
        self.websocket_buffer_overflow = Counter(
            'data_ingestion_websocket_buffer_overflow_total',
            'Number of buffer overflow events',
            ['provider', 'buffer_type']
        )
        
        # Latency tracking
        self.websocket_message_latency = Histogram(
            'data_ingestion_websocket_message_latency_milliseconds',
            'Latency from message receipt to processing completion',
            ['provider', 'message_type'],
            buckets=(1, 5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000)
        )
        
        # Heartbeat monitoring
        self.websocket_heartbeat_latency = Gauge(
            'data_ingestion_websocket_heartbeat_latency_milliseconds',
            'Latest heartbeat round-trip latency',
            ['provider', 'endpoint']
        )
        
        self.websocket_heartbeat_missed = Counter(
            'data_ingestion_websocket_heartbeat_missed_total',
            'Number of missed heartbeats',
            ['provider', 'endpoint']
        )
        
    def _init_data_flow_metrics(self):
        """Initialize data flow metrics."""
        # Data rate metrics
        self.data_ingestion_rate = Summary(
            'data_ingestion_rate_messages_per_second',
            'Rate of data ingestion per provider',
            ['provider', 'data_type', 'symbol']
        )
        
        self.data_processing_rate = Summary(
            'data_processing_rate_messages_per_second',
            'Rate of data processing',
            ['pipeline_stage', 'data_type']
        )
        
        # Volume metrics
        self.data_volume_bytes = Counter(
            'data_ingestion_volume_bytes_total',
            'Total volume of data ingested in bytes',
            ['provider', 'data_type']
        )
        
        # Symbol coverage
        self.active_symbols = Gauge(
            'data_ingestion_active_symbols',
            'Number of actively monitored symbols',
            ['provider', 'asset_class']
        )
        
        self.symbol_data_freshness = Gauge(
            'data_ingestion_symbol_data_freshness_seconds',
            'Time since last data update for symbol',
            ['provider', 'symbol', 'data_type']
        )
        
        # Data quality
        self.data_validation_errors = Counter(
            'data_ingestion_validation_errors_total',
            'Data validation errors by type',
            ['provider', 'error_type', 'severity']
        )
        
        self.data_completeness = Gauge(
            'data_ingestion_data_completeness_ratio',
            'Ratio of complete data points (0-1)',
            ['provider', 'data_type', 'time_window']
        )
        
        # Duplicate detection
        self.duplicate_messages = Counter(
            'data_ingestion_duplicate_messages_total',
            'Number of duplicate messages detected',
            ['provider', 'data_type']
        )
        
    def _init_storage_metrics(self):
        """Initialize storage and database metrics."""
        # TimescaleDB specific
        self.timescale_chunk_count = Gauge(
            'data_ingestion_timescale_chunk_count',
            'Number of TimescaleDB chunks',
            ['hypertable', 'compression_status']
        )
        
        self.timescale_compression_ratio = Gauge(
            'data_ingestion_timescale_compression_ratio',
            'Compression ratio for hypertables',
            ['hypertable']
        )
        
        self.timescale_chunk_size = Histogram(
            'data_ingestion_timescale_chunk_size_bytes',
            'Size distribution of TimescaleDB chunks',
            ['hypertable'],
            buckets=(1e6, 1e7, 5e7, 1e8, 5e8, 1e9, 5e9, 1e10)
        )
        
        # Write performance
        self.db_write_queue_depth = Gauge(
            'data_ingestion_db_write_queue_depth',
            'Current depth of database write queue',
            ['table', 'priority']
        )
        
        self.db_write_lag = Histogram(
            'data_ingestion_db_write_lag_seconds',
            'Time from data receipt to database write',
            ['table'],
            buckets=(0.1, 0.5, 1, 2, 5, 10, 30, 60)
        )
        
        # Connection pool
        self.db_connection_wait_time = Histogram(
            'data_ingestion_db_connection_wait_time_milliseconds',
            'Time waiting for database connection',
            ['pool_name'],
            buckets=(1, 5, 10, 50, 100, 500, 1000, 5000)
        )
        
    def _init_system_metrics(self):
        """Initialize system resource metrics."""
        # Memory usage
        self.process_memory_usage = Gauge(
            'data_ingestion_process_memory_usage_bytes',
            'Process memory usage',
            ['memory_type']  # heap, stack, shared
        )
        
        # CPU usage
        self.process_cpu_usage = Gauge(
            'data_ingestion_process_cpu_usage_percent',
            'Process CPU usage percentage',
            ['cpu_type']  # user, system
        )
        
        # Event loop metrics
        self.event_loop_lag = Histogram(
            'data_ingestion_event_loop_lag_milliseconds',
            'Event loop processing lag',
            ['loop_name'],
            buckets=(1, 5, 10, 50, 100, 500, 1000)
        )
        
        self.event_loop_tasks_pending = Gauge(
            'data_ingestion_event_loop_tasks_pending',
            'Number of pending tasks in event loop',
            ['loop_name']
        )
        
        # Coroutine metrics
        self.coroutine_duration = Histogram(
            'data_ingestion_coroutine_duration_seconds',
            'Duration of coroutine execution',
            ['coroutine_name'],
            buckets=(0.001, 0.01, 0.1, 0.5, 1, 5, 10, 30)
        )
        
        self.concurrent_coroutines = Gauge(
            'data_ingestion_concurrent_coroutines',
            'Number of concurrent coroutines',
            ['coroutine_type']
        )
        
    def _init_provider_metrics(self):
        """Initialize provider health metrics."""
        # Provider availability
        self.provider_availability = Gauge(
            'data_ingestion_provider_availability',
            'Provider availability score (0-1)',
            ['provider']
        )
        
        # API quota usage
        self.api_quota_usage = Gauge(
            'data_ingestion_api_quota_usage_percent',
            'API quota usage percentage',
            ['provider', 'quota_type']
        )
        
        self.api_quota_remaining = Gauge(
            'data_ingestion_api_quota_remaining',
            'Remaining API quota',
            ['provider', 'quota_type', 'reset_window']
        )
        
        # Provider errors
        self.provider_error_rate = Summary(
            'data_ingestion_provider_error_rate',
            'Provider error rate',
            ['provider', 'error_category']
        )
        
        # Data quality by provider
        self.provider_data_quality_score = Gauge(
            'data_ingestion_provider_data_quality_score',
            'Overall data quality score (0-1)',
            ['provider']
        )
        
    @asynccontextmanager
    async def track_websocket_message(self, provider: str, message_type: str):
        """Track WebSocket message processing with latency."""
        start_time = time.time()
        self.websocket_messages_received.labels(
            provider=provider,
            message_type=message_type
        ).inc()
        
        try:
            yield
            status = "success"
        except Exception as e:
            status = "error"
            logger.error(f"Error processing WebSocket message: {e}")
            raise
        finally:
            duration_ms = (time.time() - start_time) * 1000
            self.websocket_message_latency.labels(
                provider=provider,
                message_type=message_type
            ).observe(duration_ms)
            
            self.websocket_messages_processed.labels(
                provider=provider,
                message_type=message_type,
                status=status
            ).inc()
    
    def update_connection_state(self, provider: str, endpoint: str, state: int):
        """Update WebSocket connection state."""
        self.websocket_connection_state.labels(
            provider=provider,
            endpoint=endpoint
        ).set(state)
        logger.info(f"WebSocket connection state updated: {provider}/{endpoint} = {state}")
    
    def track_reconnection(self, provider: str, endpoint: str, success: bool = True):
        """Track reconnection attempt."""
        self.websocket_reconnection_attempts.labels(
            provider=provider,
            endpoint=endpoint
        ).inc()
        
        if success:
            self.websocket_reconnection_success.labels(
                provider=provider,
                endpoint=endpoint
            ).inc()
    
    def update_buffer_metrics(self, provider: str, buffer_type: str, 
                            size: int, overflow: bool = False):
        """Update buffer size and overflow metrics."""
        self.websocket_buffer_size.labels(
            provider=provider,
            buffer_type=buffer_type
        ).set(size)
        
        if overflow:
            self.websocket_buffer_overflow.labels(
                provider=provider,
                buffer_type=buffer_type
            ).inc()
    
    def track_heartbeat(self, provider: str, endpoint: str, 
                       latency_ms: Optional[float] = None, missed: bool = False):
        """Track heartbeat metrics."""
        if latency_ms is not None:
            self.websocket_heartbeat_latency.labels(
                provider=provider,
                endpoint=endpoint
            ).set(latency_ms)
        
        if missed:
            self.websocket_heartbeat_missed.labels(
                provider=provider,
                endpoint=endpoint
            ).inc()
    
    def update_data_freshness(self, provider: str, symbol: str, data_type: str):
        """Update last data update time for freshness tracking."""
        key = f"{provider}:{symbol}:{data_type}"
        self._last_update_times[key] = time.time()
    
    async def start_freshness_monitor(self, interval: int = 60):
        """Start monitoring data freshness in background."""
        async def monitor():
            while True:
                try:
                    current_time = time.time()
                    for key, last_update in self._last_update_times.items():
                        provider, symbol, data_type = key.split(':')
                        freshness = current_time - last_update
                        self.symbol_data_freshness.labels(
                            provider=provider,
                            symbol=symbol,
                            data_type=data_type
                        ).set(freshness)
                    await asyncio.sleep(interval)
                except Exception as e:
                    logger.error(f"Error in freshness monitor: {e}")
                    await asyncio.sleep(interval)
        
        if self._freshness_monitor_task is None:
            self._freshness_monitor_task = asyncio.create_task(monitor())
    
    def track_data_volume(self, provider: str, data_type: str, size_bytes: int):
        """Track data volume metrics."""
        self.data_volume_bytes.labels(
            provider=provider,
            data_type=data_type
        ).inc(size_bytes)
    
    def track_validation_error(self, provider: str, error_type: str, 
                             severity: str = "medium"):
        """Track data validation errors."""
        self.data_validation_errors.labels(
            provider=provider,
            error_type=error_type,
            severity=severity
        ).inc()
    
    def track_duplicate(self, provider: str, data_type: str):
        """Track duplicate message detection."""
        self.duplicate_messages.labels(
            provider=provider,
            data_type=data_type
        ).inc()
    
    def update_active_symbols(self, provider: str, asset_class: str, count: int):
        """Update count of active symbols."""
        self.active_symbols.labels(
            provider=provider,
            asset_class=asset_class
        ).set(count)
    
    def update_data_completeness(self, provider: str, data_type: str, 
                               time_window: str, ratio: float):
        """Update data completeness ratio (0-1)."""
        self.data_completeness.labels(
            provider=provider,
            data_type=data_type,
            time_window=time_window
        ).set(min(1.0, max(0.0, ratio)))
    
    def update_system_metrics(self):
        """Update system resource metrics."""
        try:
            # Get process info
            process = psutil.Process()
            
            # Memory metrics
            memory_info = process.memory_info()
            self.process_memory_usage.labels(memory_type='rss').set(memory_info.rss)
            self.process_memory_usage.labels(memory_type='vms').set(memory_info.vms)
            
            # CPU metrics
            cpu_percent = process.cpu_percent(interval=0.1)
            self.process_cpu_usage.labels(cpu_type='total').set(cpu_percent)
            
            # Event loop metrics
            loop = asyncio.get_event_loop()
            if hasattr(loop, '_ready'):
                pending_tasks = len(loop._ready)
                self.event_loop_tasks_pending.labels(
                    loop_name='main'
                ).set(pending_tasks)
            
            # Garbage collection stats
            gc_stats = gc.get_stats()
            if gc_stats:
                # Track generation 0 collections (most frequent)
                collections = gc_stats[0].get('collections', 0)
                collected = gc_stats[0].get('collected', 0)
                logger.debug(f"GC stats - Collections: {collections}, Collected: {collected}")
                
        except Exception as e:
            logger.error(f"Error updating system metrics: {e}")
    
    def track_coroutine(self, coroutine_name: str):
        """Decorator to track coroutine execution."""
        def decorator(func):
            @wraps(func)
            async def wrapper(*args, **kwargs):
                start_time = time.time()
                self.concurrent_coroutines.labels(
                    coroutine_type=coroutine_name
                ).inc()
                
                try:
                    result = await func(*args, **kwargs)
                    return result
                finally:
                    duration = time.time() - start_time
                    self.coroutine_duration.labels(
                        coroutine_name=coroutine_name
                    ).observe(duration)
                    self.concurrent_coroutines.labels(
                        coroutine_type=coroutine_name
                    ).dec()
            
            return wrapper
        return decorator
    
    def track_event_loop_lag(self, loop_name: str = 'main'):
        """Measure event loop lag."""
        async def measure_lag():
            start = time.time()
            await asyncio.sleep(0)  # Yield to event loop
            lag_ms = (time.time() - start) * 1000
            self.event_loop_lag.labels(loop_name=loop_name).observe(lag_ms)
        
        asyncio.create_task(measure_lag())
    
    def update_provider_health(self, provider: str, availability: float, 
                             quality_score: float):
        """Update provider health metrics."""
        self.provider_availability.labels(provider=provider).set(availability)
        self.provider_data_quality_score.labels(provider=provider).set(quality_score)
    
    def update_api_quota(self, provider: str, quota_type: str, 
                        used: int, total: int, reset_window: str = "daily"):
        """Update API quota metrics."""
        usage_percent = (used / total * 100) if total > 0 else 0
        remaining = total - used
        
        self.api_quota_usage.labels(
            provider=provider,
            quota_type=quota_type
        ).set(usage_percent)
        
        self.api_quota_remaining.labels(
            provider=provider,
            quota_type=quota_type,
            reset_window=reset_window
        ).set(remaining)
    
    @asynccontextmanager
    async def track_db_write_lag(self, table: str, receipt_time: float):
        """Track database write lag from data receipt."""
        try:
            yield
        finally:
            lag = time.time() - receipt_time
            self.db_write_lag.labels(table=table).observe(lag)
    
    def update_db_queue_depth(self, table: str, priority: str, depth: int):
        """Update database write queue depth."""
        self.db_write_queue_depth.labels(
            table=table,
            priority=priority
        ).set(depth)
    
    def track_timescale_metrics(self, hypertable: str, chunk_count: int,
                               compressed_chunks: int, compression_ratio: float,
                               chunk_sizes: list):
        """Update TimescaleDB-specific metrics."""
        self.timescale_chunk_count.labels(
            hypertable=hypertable,
            compression_status='compressed'
        ).set(compressed_chunks)
        
        self.timescale_chunk_count.labels(
            hypertable=hypertable,
            compression_status='uncompressed'
        ).set(chunk_count - compressed_chunks)
        
        self.timescale_compression_ratio.labels(
            hypertable=hypertable
        ).set(compression_ratio)
        
        for size in chunk_sizes:
            self.timescale_chunk_size.labels(
                hypertable=hypertable
            ).observe(size)


# Global enhanced metrics instance
enhanced_metrics = EnhancedMetrics()