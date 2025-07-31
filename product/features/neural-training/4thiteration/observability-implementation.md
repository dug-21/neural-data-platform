# Data Ingestion Observability Implementation

## Overview

This document outlines a comprehensive observability strategy for the data ingestion system using Prometheus metrics and Grafana dashboards. The implementation focuses on monitoring WebSocket connections, data flow rates, system health, and performance metrics.

## Architecture

### Components
1. **Prometheus Client** - Metrics collection and exposure
2. **Prometheus Server** - Time-series database for metrics
3. **Grafana** - Visualization and alerting
4. **Alert Manager** - Alert routing and notification

## Metric Categories

### 1. WebSocket Connection Metrics

```python
# Connection state tracking
websocket_connection_state = Gauge(
    'data_ingestion_websocket_connection_state',
    'WebSocket connection state (0=disconnected, 1=connecting, 2=authenticating, 3=connected, 4=reconnecting, 5=failed)',
    ['provider', 'endpoint']
)

# Connection lifecycle
websocket_connection_duration = Histogram(
    'data_ingestion_websocket_connection_duration_seconds',
    'Duration of WebSocket connections',
    ['provider', 'endpoint', 'termination_reason'],
    buckets=(60, 300, 900, 1800, 3600, 7200, 14400, 28800, 86400)
)

# Reconnection metrics
websocket_reconnection_attempts = Counter(
    'data_ingestion_websocket_reconnection_attempts_total',
    'Total number of reconnection attempts',
    ['provider', 'endpoint']
)

websocket_reconnection_success = Counter(
    'data_ingestion_websocket_reconnection_success_total',
    'Successful reconnection attempts',
    ['provider', 'endpoint']
)

# Message flow
websocket_messages_received = Counter(
    'data_ingestion_websocket_messages_received_total',
    'Total messages received via WebSocket',
    ['provider', 'message_type']
)

websocket_messages_processed = Counter(
    'data_ingestion_websocket_messages_processed_total',
    'Total messages successfully processed',
    ['provider', 'message_type', 'status']
)

# Buffer metrics
websocket_buffer_size = Gauge(
    'data_ingestion_websocket_buffer_size',
    'Current WebSocket message buffer size',
    ['provider', 'buffer_type']
)

websocket_buffer_overflow = Counter(
    'data_ingestion_websocket_buffer_overflow_total',
    'Number of buffer overflow events',
    ['provider', 'buffer_type']
)

# Latency tracking
websocket_message_latency = Histogram(
    'data_ingestion_websocket_message_latency_milliseconds',
    'Latency from message receipt to processing completion',
    ['provider', 'message_type'],
    buckets=(1, 5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000)
)

# Heartbeat monitoring
websocket_heartbeat_latency = Gauge(
    'data_ingestion_websocket_heartbeat_latency_milliseconds',
    'Latest heartbeat round-trip latency',
    ['provider', 'endpoint']
)

websocket_heartbeat_missed = Counter(
    'data_ingestion_websocket_heartbeat_missed_total',
    'Number of missed heartbeats',
    ['provider', 'endpoint']
)
```

### 2. Data Flow Metrics

```python
# Data rate metrics
data_ingestion_rate = Summary(
    'data_ingestion_rate_messages_per_second',
    'Rate of data ingestion per provider',
    ['provider', 'data_type', 'symbol']
)

data_processing_rate = Summary(
    'data_processing_rate_messages_per_second',
    'Rate of data processing',
    ['pipeline_stage', 'data_type']
)

# Volume metrics
data_volume_bytes = Counter(
    'data_ingestion_volume_bytes_total',
    'Total volume of data ingested in bytes',
    ['provider', 'data_type']
)

# Symbol coverage
active_symbols = Gauge(
    'data_ingestion_active_symbols',
    'Number of actively monitored symbols',
    ['provider', 'asset_class']
)

symbol_data_freshness = Gauge(
    'data_ingestion_symbol_data_freshness_seconds',
    'Time since last data update for symbol',
    ['provider', 'symbol', 'data_type']
)

# Data quality
data_validation_errors = Counter(
    'data_ingestion_validation_errors_total',
    'Data validation errors by type',
    ['provider', 'error_type', 'severity']
)

data_completeness = Gauge(
    'data_ingestion_data_completeness_ratio',
    'Ratio of complete data points (0-1)',
    ['provider', 'data_type', 'time_window']
)

# Duplicate detection
duplicate_messages = Counter(
    'data_ingestion_duplicate_messages_total',
    'Number of duplicate messages detected',
    ['provider', 'data_type']
)
```

### 3. Storage and Database Metrics

```python
# TimescaleDB specific
timescale_chunk_count = Gauge(
    'data_ingestion_timescale_chunk_count',
    'Number of TimescaleDB chunks',
    ['hypertable', 'compression_status']
)

timescale_compression_ratio = Gauge(
    'data_ingestion_timescale_compression_ratio',
    'Compression ratio for hypertables',
    ['hypertable']
)

timescale_chunk_size = Histogram(
    'data_ingestion_timescale_chunk_size_bytes',
    'Size distribution of TimescaleDB chunks',
    ['hypertable'],
    buckets=(1e6, 1e7, 5e7, 1e8, 5e8, 1e9, 5e9, 1e10)
)

# Write performance
db_write_queue_depth = Gauge(
    'data_ingestion_db_write_queue_depth',
    'Current depth of database write queue',
    ['table', 'priority']
)

db_write_lag = Histogram(
    'data_ingestion_db_write_lag_seconds',
    'Time from data receipt to database write',
    ['table'],
    buckets=(0.1, 0.5, 1, 2, 5, 10, 30, 60)
)

# Connection pool
db_connection_wait_time = Histogram(
    'data_ingestion_db_connection_wait_time_milliseconds',
    'Time waiting for database connection',
    ['pool_name'],
    buckets=(1, 5, 10, 50, 100, 500, 1000, 5000)
)
```

### 4. System Resource Metrics

```python
# Memory usage
process_memory_usage = Gauge(
    'data_ingestion_process_memory_usage_bytes',
    'Process memory usage',
    ['memory_type']  # heap, stack, shared
)

# CPU usage
process_cpu_usage = Gauge(
    'data_ingestion_process_cpu_usage_percent',
    'Process CPU usage percentage',
    ['cpu_type']  # user, system
)

# Event loop metrics
event_loop_lag = Histogram(
    'data_ingestion_event_loop_lag_milliseconds',
    'Event loop processing lag',
    ['loop_name'],
    buckets=(1, 5, 10, 50, 100, 500, 1000)
)

event_loop_tasks_pending = Gauge(
    'data_ingestion_event_loop_tasks_pending',
    'Number of pending tasks in event loop',
    ['loop_name']
)

# Coroutine metrics
coroutine_duration = Histogram(
    'data_ingestion_coroutine_duration_seconds',
    'Duration of coroutine execution',
    ['coroutine_name'],
    buckets=(0.001, 0.01, 0.1, 0.5, 1, 5, 10, 30)
)

concurrent_coroutines = Gauge(
    'data_ingestion_concurrent_coroutines',
    'Number of concurrent coroutines',
    ['coroutine_type']
)
```

### 5. Provider Health Metrics

```python
# Provider availability
provider_availability = Gauge(
    'data_ingestion_provider_availability',
    'Provider availability score (0-1)',
    ['provider']
)

# API quota usage
api_quota_usage = Gauge(
    'data_ingestion_api_quota_usage_percent',
    'API quota usage percentage',
    ['provider', 'quota_type']
)

api_quota_remaining = Gauge(
    'data_ingestion_api_quota_remaining',
    'Remaining API quota',
    ['provider', 'quota_type', 'reset_window']
)

# Provider errors
provider_error_rate = Summary(
    'data_ingestion_provider_error_rate',
    'Provider error rate',
    ['provider', 'error_category']
)

# Data quality by provider
provider_data_quality_score = Gauge(
    'data_ingestion_provider_data_quality_score',
    'Overall data quality score (0-1)',
    ['provider']
)
```

## Implementation Details

### 1. Metric Collection Integration

```python
# utils/metrics_enhanced.py
from prometheus_client import Counter, Histogram, Gauge, Summary
from contextlib import asynccontextmanager
import time
import asyncio
from typing import Optional, Dict, Any

class EnhancedMetrics:
    """Enhanced metrics collection for observability."""
    
    def __init__(self):
        # Initialize all metrics defined above
        self._init_websocket_metrics()
        self._init_data_flow_metrics()
        self._init_storage_metrics()
        self._init_system_metrics()
        self._init_provider_metrics()
    
    @asynccontextmanager
    async def track_websocket_message(self, provider: str, message_type: str):
        """Track WebSocket message processing."""
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
    
    async def track_data_freshness(self, provider: str, symbol: str, data_type: str):
        """Track data freshness for symbols."""
        while True:
            last_update = await self._get_last_update_time(provider, symbol, data_type)
            if last_update:
                freshness = time.time() - last_update
                self.symbol_data_freshness.labels(
                    provider=provider,
                    symbol=symbol,
                    data_type=data_type
                ).set(freshness)
            await asyncio.sleep(60)  # Update every minute
```

### 2. Metric Naming Conventions

All metrics follow the pattern: `data_ingestion_<component>_<metric>_<unit>`

- **Component**: websocket, provider, storage, system
- **Metric**: descriptive name (e.g., connection_state, message_latency)
- **Unit**: seconds, bytes, total, percent, ratio

### 3. Label Strategies

Labels are used for dimensional data but kept minimal to avoid cardinality explosion:

- **provider**: Limited set of data providers
- **data_type**: trade, quote, aggregate, orderbook
- **status**: success, error, timeout
- **message_type**: Specific message types per provider
- **priority**: high, medium, low

## Grafana Dashboard Designs

### 1. WebSocket Connection Health Dashboard

```json
{
  "title": "WebSocket Connection Health",
  "panels": [
    {
      "title": "Connection Status by Provider",
      "type": "stat",
      "targets": [{
        "expr": "data_ingestion_websocket_connection_state"
      }]
    },
    {
      "title": "Message Flow Rate",
      "type": "graph",
      "targets": [{
        "expr": "rate(data_ingestion_websocket_messages_received_total[5m])"
      }]
    },
    {
      "title": "Message Processing Latency",
      "type": "heatmap",
      "targets": [{
        "expr": "data_ingestion_websocket_message_latency_milliseconds"
      }]
    },
    {
      "title": "Reconnection Success Rate",
      "type": "gauge",
      "targets": [{
        "expr": "rate(data_ingestion_websocket_reconnection_success_total[1h]) / rate(data_ingestion_websocket_reconnection_attempts_total[1h])"
      }]
    }
  ]
}
```

### 2. Data Flow Dashboard

```json
{
  "title": "Data Flow Monitoring",
  "panels": [
    {
      "title": "Ingestion Rate by Provider",
      "type": "graph",
      "targets": [{
        "expr": "data_ingestion_rate_messages_per_second"
      }]
    },
    {
      "title": "Active Symbols",
      "type": "stat",
      "targets": [{
        "expr": "sum(data_ingestion_active_symbols)"
      }]
    },
    {
      "title": "Data Freshness Heatmap",
      "type": "heatmap",
      "targets": [{
        "expr": "data_ingestion_symbol_data_freshness_seconds"
      }]
    },
    {
      "title": "Validation Error Rate",
      "type": "graph",
      "targets": [{
        "expr": "rate(data_ingestion_validation_errors_total[5m])"
      }]
    }
  ]
}
```

### 3. System Performance Dashboard

```json
{
  "title": "System Performance",
  "panels": [
    {
      "title": "Memory Usage",
      "type": "graph",
      "targets": [{
        "expr": "data_ingestion_process_memory_usage_bytes"
      }]
    },
    {
      "title": "Event Loop Lag",
      "type": "graph",
      "targets": [{
        "expr": "histogram_quantile(0.99, data_ingestion_event_loop_lag_milliseconds)"
      }]
    },
    {
      "title": "Database Write Performance",
      "type": "graph",
      "targets": [{
        "expr": "data_ingestion_db_write_throughput_rows_per_second"
      }]
    },
    {
      "title": "Connection Pool Usage",
      "type": "gauge",
      "targets": [{
        "expr": "data_ingestion_db_connection_pool_size{state='active'} / data_ingestion_db_connection_pool_size{state='total'}"
      }]
    }
  ]
}
```

## Alert Rules

### Critical Alerts

```yaml
groups:
  - name: data_ingestion_critical
    rules:
      - alert: WebSocketConnectionDown
        expr: data_ingestion_websocket_connection_state != 3
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "WebSocket connection down for {{ $labels.provider }}"
          description: "Connection has been down for more than 5 minutes"
      
      - alert: DataStaleness
        expr: data_ingestion_symbol_data_freshness_seconds > 300
        for: 10m
        labels:
          severity: critical
        annotations:
          summary: "Stale data for {{ $labels.symbol }}"
          description: "No updates received for more than 5 minutes"
      
      - alert: DatabaseWriteBacklog
        expr: data_ingestion_db_write_queue_depth > 10000
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Database write backlog critical"
          description: "Write queue depth exceeds 10k items"
```

### Warning Alerts

```yaml
  - name: data_ingestion_warnings
    rules:
      - alert: HighErrorRate
        expr: rate(data_ingestion_processing_errors_total[5m]) > 10
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High error rate for {{ $labels.provider }}"
          description: "Error rate exceeds 10/sec"
      
      - alert: MemoryUsageHigh
        expr: data_ingestion_process_memory_usage_bytes / 1e9 > 4
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "High memory usage"
          description: "Process using more than 4GB RAM"
      
      - alert: EventLoopLag
        expr: histogram_quantile(0.99, data_ingestion_event_loop_lag_milliseconds) > 100
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Event loop lagging"
          description: "99th percentile lag exceeds 100ms"
```

## Implementation Steps

1. **Phase 1: Core Metrics**
   - Implement WebSocket connection tracking
   - Add basic data flow metrics
   - Set up Prometheus endpoint

2. **Phase 2: Enhanced Monitoring**
   - Add detailed latency tracking
   - Implement buffer monitoring
   - Add provider health scoring

3. **Phase 3: Dashboards**
   - Create Grafana dashboards
   - Set up alert rules
   - Configure notification channels

4. **Phase 4: Advanced Analytics**
   - Implement anomaly detection
   - Add predictive alerts
   - Create SLI/SLO tracking

## Testing Strategy

### Metric Validation
```python
async def test_websocket_metrics():
    """Validate WebSocket metrics are properly collected."""
    # Simulate connection lifecycle
    metrics.update_connection_state("test_provider", "test_endpoint", 1)
    
    # Simulate message processing
    async with metrics.track_websocket_message("test_provider", "trade"):
        await asyncio.sleep(0.01)  # Simulate processing
    
    # Verify metrics
    assert metrics.websocket_messages_received._value.get() > 0
    assert metrics.websocket_message_latency._sum.get() > 0
```

### Load Testing
- Simulate high message rates
- Test metric collection overhead
- Validate metric accuracy under load

## Performance Considerations

1. **Metric Cardinality**
   - Limit label combinations
   - Use bounded values for dynamic labels
   - Implement label sanitization

2. **Collection Overhead**
   - Use sampling for high-frequency metrics
   - Batch metric updates
   - Implement async metric collection

3. **Storage Optimization**
   - Configure appropriate retention policies
   - Use metric aggregation rules
   - Implement downsampling for historical data

## Security Considerations

1. **Metric Exposure**
   - Secure Prometheus endpoint with authentication
   - Limit metric access by role
   - Sanitize sensitive data in labels

2. **Alert Security**
   - Encrypt alert notifications
   - Implement alert acknowledgment
   - Audit alert configuration changes

## Maintenance

1. **Regular Reviews**
   - Monthly dashboard review
   - Quarterly alert threshold tuning
   - Annual metric inventory cleanup

2. **Documentation**
   - Maintain metric catalog
   - Document alert runbooks
   - Keep dashboard screenshots updated

## Conclusion

This observability implementation provides comprehensive monitoring for the data ingestion system, enabling proactive issue detection, performance optimization, and system reliability improvements. The metrics and dashboards will evolve based on operational experience and changing requirements.