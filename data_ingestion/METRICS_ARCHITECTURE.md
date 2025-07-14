# Comprehensive Metrics Architecture for Data Ingestion System

## Overview

This document outlines a comprehensive metrics architecture for the Neural Trader data ingestion system, following Prometheus best practices and providing multi-dimensional analysis capabilities.

## Current State Analysis

### Existing Metrics

The system currently implements the following metrics in `utils/metrics.py`:

1. **API Request Metrics**
   - `data_ingestion_api_requests_total` - Counter with labels: provider, endpoint, status
   - `data_ingestion_api_request_duration_seconds` - Histogram with labels: provider, endpoint

2. **Data Processing Metrics**
   - `data_ingestion_points_processed_total` - Counter with labels: provider, data_type
   - `data_ingestion_processing_errors_total` - Counter with labels: provider, error_type
   - `data_ingestion_processing_errors_by_stage_total` - Counter with labels: stage

3. **Storage Metrics**
   - `data_ingestion_storage_operations_total` - Counter with labels: storage_type, operation, status
   - `data_ingestion_storage_duration_seconds` - Histogram with labels: storage_type, operation

4. **System Metrics**
   - `data_ingestion_active_connections` - Gauge with labels: connection_type
   - `data_ingestion_queue_size` - Gauge with labels: queue_name

5. **Rate Limiting & Streaming**
   - `data_ingestion_rate_limit_hits_total` - Counter with labels: provider
   - `data_ingestion_streaming_errors_total` - Counter with labels: provider
   - `data_ingestion_active_streams` - Gauge (no labels)

6. **Data Quality**
   - `data_ingestion_validation_failures_total` - Counter with labels: provider
   - `data_ingestion_data_quality_issues_total` - Counter with labels: issue_type

7. **Batch Processing**
   - `data_ingestion_batch_job_duration_seconds` - Histogram with labels: job_id
   - `data_ingestion_batch_job_errors_total` - Counter with labels: job_id, provider
   - `data_ingestion_batch_job_success_total` - Counter with labels: job_id

## Proposed Metrics Architecture

### 1. Provider Health Metrics

```python
# Connection lifecycle metrics
provider_connection_status = Gauge(
    'data_ingestion_provider_connection_status',
    'Current connection status of provider (1=connected, 0=disconnected)',
    ['provider']
)

provider_reconnection_attempts_total = Counter(
    'data_ingestion_provider_reconnection_attempts_total',
    'Total number of reconnection attempts',
    ['provider', 'reason']
)

provider_connection_duration_seconds = Histogram(
    'data_ingestion_provider_connection_duration_seconds',
    'Duration of active connections',
    ['provider'],
    buckets=[60, 300, 900, 3600, 7200, 14400, 28800]  # 1m, 5m, 15m, 1h, 2h, 4h, 8h
)

# API availability
provider_api_availability = Gauge(
    'data_ingestion_provider_api_availability',
    'API endpoint availability (1=available, 0=unavailable)',
    ['provider', 'endpoint']
)

provider_api_error_rate = Gauge(
    'data_ingestion_provider_api_error_rate',
    'Current error rate for API calls (rolling 5-minute window)',
    ['provider', 'endpoint']
)
```

### 2. WebSocket Metrics

```python
# WebSocket connection metrics
websocket_connection_status = Gauge(
    'data_ingestion_websocket_connection_status',
    'WebSocket connection status (1=connected, 0=disconnected)',
    ['provider', 'stream_type']  # stream_type: bars, trades, quotes
)

websocket_messages_received_total = Counter(
    'data_ingestion_websocket_messages_received_total',
    'Total number of WebSocket messages received',
    ['provider', 'message_type', 'symbol']
)

websocket_message_rate = Gauge(
    'data_ingestion_websocket_message_rate',
    'Current message rate per second (rolling window)',
    ['provider', 'message_type']
)

websocket_reconnections_total = Counter(
    'data_ingestion_websocket_reconnections_total',
    'Total number of WebSocket reconnections',
    ['provider', 'reason']
)

websocket_latency_seconds = Histogram(
    'data_ingestion_websocket_latency_seconds',
    'Latency between message timestamp and processing time',
    ['provider', 'message_type'],
    buckets=[0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]
)

websocket_subscription_count = Gauge(
    'data_ingestion_websocket_subscription_count',
    'Number of active subscriptions',
    ['provider', 'subscription_type']
)
```

### 3. Redis Publish Metrics

```python
# Redis publish metrics
redis_publish_success_total = Counter(
    'data_ingestion_redis_publish_success_total',
    'Total successful Redis publishes',
    ['channel_type', 'data_type']  # channel_type: price_updates, tick_updates, orderbook_updates
)

redis_publish_failures_total = Counter(
    'data_ingestion_redis_publish_failures_total',
    'Total failed Redis publishes',
    ['channel_type', 'error_type']
)

redis_publish_latency_seconds = Histogram(
    'data_ingestion_redis_publish_latency_seconds',
    'Time taken to publish to Redis',
    ['channel_type'],
    buckets=[0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]
)

redis_channel_subscribers = Gauge(
    'data_ingestion_redis_channel_subscribers',
    'Number of subscribers to Redis channels',
    ['channel_pattern']
)

redis_memory_usage_bytes = Gauge(
    'data_ingestion_redis_memory_usage_bytes',
    'Redis memory usage for data ingestion keys',
    ['key_pattern']
)
```

### 4. Database Write Metrics

```python
# Database write metrics
db_insert_batch_size = Histogram(
    'data_ingestion_db_insert_batch_size',
    'Size of insert batches',
    ['table_name'],
    buckets=[1, 10, 50, 100, 500, 1000, 5000, 10000]
)

db_insert_conflicts_total = Counter(
    'data_ingestion_db_insert_conflicts_total',
    'Total number of insert conflicts',
    ['table_name', 'conflict_type']  # conflict_type: duplicate_key, constraint_violation
)

db_constraint_violations_total = Counter(
    'data_ingestion_db_constraint_violations_total',
    'Total number of constraint violations',
    ['table_name', 'constraint_name']
)

db_rows_inserted_total = Counter(
    'data_ingestion_db_rows_inserted_total',
    'Total number of rows successfully inserted',
    ['table_name', 'provider']
)

db_write_latency_seconds = Histogram(
    'data_ingestion_db_write_latency_seconds',
    'Database write latency',
    ['table_name', 'operation'],
    buckets=[0.01, 0.05, 0.1, 0.5, 1.0, 2.5, 5.0, 10.0]
)

db_connection_pool_usage = Gauge(
    'data_ingestion_db_connection_pool_usage',
    'Database connection pool usage',
    ['pool_name', 'state']  # state: active, idle, waiting
)
```

### 5. Symbol-Level Metrics

```python
# Symbol-level metrics
symbol_update_frequency = Histogram(
    'data_ingestion_symbol_update_frequency_seconds',
    'Time between updates for a symbol',
    ['symbol', 'provider', 'data_type'],
    buckets=[0.1, 1, 5, 10, 30, 60, 300, 600, 3600]
)

symbol_data_gaps_total = Counter(
    'data_ingestion_symbol_data_gaps_total',
    'Total number of detected data gaps',
    ['symbol', 'provider', 'gap_duration_bucket']
)

symbol_data_quality_score = Gauge(
    'data_ingestion_symbol_data_quality_score',
    'Data quality score for symbol (0-1)',
    ['symbol', 'provider']
)

symbol_last_update_timestamp = Gauge(
    'data_ingestion_symbol_last_update_timestamp',
    'Unix timestamp of last update for symbol',
    ['symbol', 'provider', 'data_type']
)

symbol_data_volume_bytes = Counter(
    'data_ingestion_symbol_data_volume_bytes',
    'Total data volume processed for symbol',
    ['symbol', 'provider']
)
```

### 6. Data Pipeline Metrics

```python
# Pipeline stage metrics
pipeline_stage_duration_seconds = Histogram(
    'data_ingestion_pipeline_stage_duration_seconds',
    'Duration of each pipeline stage',
    ['stage', 'provider'],  # stage: fetch, validate, transform, store, publish
    buckets=[0.001, 0.01, 0.1, 1.0, 10.0]
)

pipeline_stage_errors_total = Counter(
    'data_ingestion_pipeline_stage_errors_total',
    'Errors by pipeline stage',
    ['stage', 'provider', 'error_category']
)

pipeline_throughput_items_per_second = Gauge(
    'data_ingestion_pipeline_throughput_items_per_second',
    'Current pipeline throughput',
    ['provider', 'data_type']
)

pipeline_backpressure = Gauge(
    'data_ingestion_pipeline_backpressure',
    'Current backpressure level (0-1)',
    ['provider', 'stage']
)
```

### 7. Resource Utilization Metrics

```python
# Resource metrics
resource_cpu_usage_percent = Gauge(
    'data_ingestion_resource_cpu_usage_percent',
    'CPU usage percentage',
    ['component']  # component: provider_alpaca, storage_redis, storage_timescale
)

resource_memory_usage_bytes = Gauge(
    'data_ingestion_resource_memory_usage_bytes',
    'Memory usage in bytes',
    ['component']
)

resource_goroutines_count = Gauge(
    'data_ingestion_resource_goroutines_count',
    'Number of active goroutines/tasks',
    ['component']
)

resource_open_file_descriptors = Gauge(
    'data_ingestion_resource_open_file_descriptors',
    'Number of open file descriptors',
    ['component']
)
```

## Metric Naming Conventions

Following Prometheus best practices:

1. **Prefix**: All metrics start with `data_ingestion_`
2. **Units**: Include units in metric names (_seconds, _bytes, _total, _percent)
3. **Suffixes**:
   - `_total` for monotonic counters
   - `_seconds` for durations
   - `_bytes` for sizes
   - `_percent` for percentages (0-100)
   - `_ratio` for ratios (0-1)

## Label Design Principles

1. **Cardinality Control**: Limit label values to prevent metric explosion
   - Use bounded sets (e.g., provider names, not user IDs)
   - Group similar errors into categories
   
2. **Consistency**: Use consistent label names across metrics
   - `provider`: Data provider name (alpaca, polygon, etc.)
   - `symbol`: Stock symbol (controlled set)
   - `data_type`: Type of data (market_data, tick_data, order_book)
   - `status`: Operation status (success, error, timeout)
   
3. **Hierarchical Labels**: Order from most to least specific
   - Good: provider → endpoint → status
   - Bad: status → endpoint → provider

## Implementation Guidelines

### 1. Decorator Pattern Enhancement

```python
def track_provider_operation(provider: str, operation: str):
    """Enhanced decorator for provider operations."""
    def decorator(func):
        @wraps(func)
        async def wrapper(*args, **kwargs):
            start_time = time.time()
            status = "success"
            error_type = None
            
            try:
                # Pre-operation metrics
                metrics.pipeline_stage_duration_seconds.labels(
                    stage="start", provider=provider
                ).observe(0)
                
                result = await func(*args, **kwargs)
                return result
                
            except RateLimitError as e:
                status = "rate_limited"
                error_type = "rate_limit"
                metrics.rate_limit_hits.labels(provider=provider).inc()
                raise
                
            except ConnectionError as e:
                status = "connection_error"
                error_type = "connection"
                metrics.provider_reconnection_attempts_total.labels(
                    provider=provider, reason="connection_error"
                ).inc()
                raise
                
            except Exception as e:
                status = "error"
                error_type = type(e).__name__
                raise
                
            finally:
                duration = time.time() - start_time
                
                # Record operation metrics
                metrics.api_requests_total.labels(
                    provider=provider,
                    endpoint=operation,
                    status=status
                ).inc()
                
                metrics.api_request_duration.labels(
                    provider=provider,
                    endpoint=operation
                ).observe(duration)
                
                if error_type:
                    metrics.pipeline_stage_errors_total.labels(
                        stage=operation,
                        provider=provider,
                        error_category=error_type
                    ).inc()
        
        return wrapper
    return decorator
```

### 2. Context Manager for Connection Tracking

```python
class ConnectionMetrics:
    """Context manager for tracking connection metrics."""
    
    def __init__(self, provider: str, connection_type: str):
        self.provider = provider
        self.connection_type = connection_type
        self.start_time = None
        
    async def __aenter__(self):
        self.start_time = time.time()
        metrics.provider_connection_status.labels(
            provider=self.provider
        ).set(1)
        metrics.active_connections.labels(
            connection_type=self.connection_type
        ).inc()
        return self
        
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        duration = time.time() - self.start_time
        
        metrics.provider_connection_status.labels(
            provider=self.provider
        ).set(0)
        
        metrics.active_connections.labels(
            connection_type=self.connection_type
        ).dec()
        
        metrics.provider_connection_duration_seconds.labels(
            provider=self.provider
        ).observe(duration)
        
        if exc_type:
            metrics.provider_reconnection_attempts_total.labels(
                provider=self.provider,
                reason=exc_type.__name__
            ).inc()
```

### 3. WebSocket Metrics Integration

```python
class WebSocketMetrics:
    """WebSocket-specific metrics tracking."""
    
    def __init__(self, provider: str):
        self.provider = provider
        self.message_timestamps = defaultdict(deque)
        self.last_rate_calculation = time.time()
        
    async def on_connect(self):
        metrics.websocket_connection_status.labels(
            provider=self.provider,
            stream_type="market_data"
        ).set(1)
        
    async def on_disconnect(self, reason: str):
        metrics.websocket_connection_status.labels(
            provider=self.provider,
            stream_type="market_data"
        ).set(0)
        
        metrics.websocket_reconnections_total.labels(
            provider=self.provider,
            reason=reason
        ).inc()
        
    async def on_message(self, message_type: str, symbol: str, data: dict):
        # Track message count
        metrics.websocket_messages_received_total.labels(
            provider=self.provider,
            message_type=message_type,
            symbol=symbol
        ).inc()
        
        # Calculate latency
        if 'timestamp' in data:
            latency = time.time() - data['timestamp']
            metrics.websocket_latency_seconds.labels(
                provider=self.provider,
                message_type=message_type
            ).observe(latency)
        
        # Update message rate
        self._update_message_rate(message_type)
        
    def _update_message_rate(self, message_type: str):
        now = time.time()
        self.message_timestamps[message_type].append(now)
        
        # Keep only last 5 seconds of timestamps
        cutoff = now - 5
        while self.message_timestamps[message_type] and \
              self.message_timestamps[message_type][0] < cutoff:
            self.message_timestamps[message_type].popleft()
        
        # Update rate gauge every second
        if now - self.last_rate_calculation > 1:
            for msg_type, timestamps in self.message_timestamps.items():
                rate = len(timestamps) / 5.0  # messages per second
                metrics.websocket_message_rate.labels(
                    provider=self.provider,
                    message_type=msg_type
                ).set(rate)
            self.last_rate_calculation = now
```

### 4. Symbol-Level Tracking

```python
class SymbolMetrics:
    """Track metrics at the symbol level."""
    
    def __init__(self):
        self.last_update_times = defaultdict(dict)
        self.quality_scores = defaultdict(lambda: 1.0)
        
    async def record_update(self, symbol: str, provider: str, data_type: str):
        now = time.time()
        key = (symbol, provider, data_type)
        
        # Update frequency tracking
        if key in self.last_update_times:
            gap = now - self.last_update_times[key]
            metrics.symbol_update_frequency.labels(
                symbol=symbol,
                provider=provider,
                data_type=data_type
            ).observe(gap)
            
            # Detect gaps
            if gap > 60:  # More than 1 minute gap
                bucket = self._get_gap_bucket(gap)
                metrics.symbol_data_gaps_total.labels(
                    symbol=symbol,
                    provider=provider,
                    gap_duration_bucket=bucket
                ).inc()
                
                # Adjust quality score
                self.quality_scores[key] *= 0.95
        
        self.last_update_times[key] = now
        
        # Update last timestamp gauge
        metrics.symbol_last_update_timestamp.labels(
            symbol=symbol,
            provider=provider,
            data_type=data_type
        ).set(now)
        
        # Update quality score
        metrics.symbol_data_quality_score.labels(
            symbol=symbol,
            provider=provider
        ).set(self.quality_scores[key])
        
    def _get_gap_bucket(self, gap_seconds: float) -> str:
        if gap_seconds < 300:
            return "1-5min"
        elif gap_seconds < 900:
            return "5-15min"
        elif gap_seconds < 3600:
            return "15-60min"
        else:
            return ">60min"
```

## Monitoring Dashboards

### 1. Provider Health Dashboard
- Connection status heatmap
- API error rates by provider/endpoint
- Reconnection attempts timeline
- Connection duration distribution

### 2. Real-Time Data Flow Dashboard
- WebSocket message rates
- Message latency percentiles
- Active subscriptions
- Data gaps visualization

### 3. Storage Performance Dashboard
- Redis publish success/failure rates
- Database write throughput
- Storage latency percentiles
- Connection pool utilization

### 4. Symbol Coverage Dashboard
- Update frequency heatmap (symbol × provider)
- Data quality scores
- Gap detection alerts
- Volume processed by symbol

### 5. System Overview Dashboard
- Pipeline stage performance
- Resource utilization
- Error rates by category
- Throughput trends

## Alert Rules

### Critical Alerts

```yaml
# Provider completely down
- alert: ProviderConnectionDown
  expr: data_ingestion_provider_connection_status == 0
  for: 5m
  annotations:
    summary: "Provider {{ $labels.provider }} connection down"
    
# High error rate
- alert: HighAPIErrorRate
  expr: rate(data_ingestion_api_requests_total{status="error"}[5m]) > 0.1
  annotations:
    summary: "High error rate for {{ $labels.provider }}/{{ $labels.endpoint }}"
    
# Data gaps detected
- alert: DataGapsDetected
  expr: increase(data_ingestion_symbol_data_gaps_total[5m]) > 0
  annotations:
    summary: "Data gaps detected for {{ $labels.symbol }} from {{ $labels.provider }}"
```

### Warning Alerts

```yaml
# Degraded performance
- alert: SlowStorageOperations
  expr: histogram_quantile(0.95, data_ingestion_storage_duration_seconds) > 1
  annotations:
    summary: "Slow storage operations for {{ $labels.storage_type }}"
    
# Connection pool exhaustion
- alert: ConnectionPoolNearCapacity
  expr: data_ingestion_db_connection_pool_usage{state="active"} / data_ingestion_db_connection_pool_usage{state="total"} > 0.8
  annotations:
    summary: "Database connection pool near capacity"
```

## Implementation Checklist

1. **Phase 1: Core Provider Metrics**
   - [ ] Provider connection tracking
   - [ ] Enhanced error categorization
   - [ ] API availability monitoring

2. **Phase 2: WebSocket Metrics**
   - [ ] Connection status tracking
   - [ ] Message rate monitoring
   - [ ] Latency measurements

3. **Phase 3: Storage Metrics**
   - [ ] Redis publish tracking
   - [ ] Database write monitoring
   - [ ] Connection pool metrics

4. **Phase 4: Symbol-Level Metrics**
   - [ ] Update frequency tracking
   - [ ] Gap detection
   - [ ] Quality scoring

5. **Phase 5: Dashboards & Alerts**
   - [ ] Create Grafana dashboards
   - [ ] Configure alert rules
   - [ ] Set up notification channels

## Testing Strategy

1. **Unit Tests**: Test metric recording logic
2. **Integration Tests**: Verify metrics in data flow
3. **Load Tests**: Validate metrics under stress
4. **Monitoring Tests**: Ensure dashboards work correctly

## Performance Considerations

1. **Label Cardinality**: Keep symbol set bounded
2. **Histogram Buckets**: Tune for actual latencies
3. **Gauge Updates**: Rate-limit frequent updates
4. **Memory Usage**: Monitor Prometheus memory usage

## Conclusion

This comprehensive metrics architecture provides deep visibility into the data ingestion system's health, performance, and reliability. By implementing these metrics, we can:

- Detect and respond to issues quickly
- Optimize system performance
- Ensure data quality and completeness
- Make data-driven decisions about system improvements