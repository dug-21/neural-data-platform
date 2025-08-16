# Neural Trader Prometheus Metrics Exposure Analysis

## Executive Summary

The neural-trader application exposes Prometheus metrics through a dedicated health server on port **9092** at the `/metrics` endpoint. The application uses a hybrid metrics approach combining custom business metrics with system monitoring capabilities.

## Metrics Infrastructure

### Core Libraries Used
- `metrics = "0.24"` - Core metrics abstractions
- `metrics-exporter-prometheus = "0.16"` - Prometheus export functionality  
- `prometheus = "0.13"` - Native Prometheus client
- Custom simplified metrics implementation for fallback

### Exposure Configuration
- **Port**: 9092 (configurable via `METRICS_PORT` environment variable)
- **Path**: `/metrics`
- **Format**: Prometheus text format (version 0.0.4)
- **Access**: Exposed only to localhost (127.0.0.1:9092) in production
- **Server**: Axum-based health server with metrics endpoint

## Metric Categories and Names

### 1. Business/Trading Metrics

#### Prediction Metrics
```prometheus
# Neural model predictions
neural_trader_predictions_by_model_total{model="LSTM"}
neural_trader_predictions_by_model_total{model="MLP"} 
neural_trader_predictions_by_model_total{model="Transformer"}
neural_trader_successful_trades_total
```

#### Performance Metrics
```prometheus
# Model inference timing
operation_duration_seconds{operation="model_inference"}
operation_duration_seconds{operation="http_request"}
operation_duration_seconds{operation="db_query"}
```

### 2. System Health Metrics

#### Component Health Status
```prometheus
# Overall system health (0.0-1.0)
system_health_score

# Component status (1=healthy, 0.5=degraded, 0=unhealthy)
component_health_status{component="Database"} 
component_health_status{component="Redis"}
component_health_status{component="NeuralSystem"}
component_health_status{component="DAAOrchestrator"}

# Component counts
healthy_components_total
unhealthy_components_total
degraded_components_total
```

#### Component Performance
```prometheus
# Response time monitoring
component_response_time

# Error tracking
component_errors_total
component_health_checks_total
```

### 3. Neural System Metrics

#### Model Storage and Availability
```prometheus
# Model management
neural_trader_models_available
neural_trader_required_models_missing
neural_trader_corrupted_models

# Storage health
neural_trader_model_storage_mounted
neural_trader_model_storage_writable
neural_trader_model_storage_size_mb
neural_trader_model_storage_disk_available_gb
neural_trader_model_storage_disk_used_percent
```

### 4. System Resource Metrics

#### CPU and Memory
```prometheus
# System resource utilization
cpu_usage_percent
memory_usage_percent
disk_usage_percent

# Load averages
cpu_load_1m
cpu_load_5m  
cpu_load_15m
```

#### Network and Storage
```prometheus
# Network throughput
network_bytes_sent
network_bytes_received
network_packets_sent
network_packets_received
network_errors

# Disk I/O
disk_io_read_bytes
disk_io_write_bytes
```

### 5. Application Performance Metrics

#### HTTP and Database
```prometheus
# Web server metrics
http_requests_total
http_request_duration
http_requests_in_flight

# Database performance
database_connections_active
database_connections_max
database_query_duration
database_queries_total
database_errors
```

#### Cache Performance
```prometheus
# Redis cache metrics
cache_hits
cache_misses
cache_hit_ratio
cache_size_bytes
```

#### Reliability Metrics
```prometheus
# Error tracking
errors_total
panics_total

# Uptime tracking
uptime_seconds
health_server_uptime_seconds
```

## Metric Naming Conventions

### Prefixes Used
- `neural_trader_*` - Business-specific trading metrics
- `component_*` - Component health and monitoring
- `system_*` - Overall system health metrics
- No prefix - Standard application metrics (http, database, cache, etc.)

### Label Strategy
- **component**: Identifies system components (Database, Redis, NeuralSystem, DAAOrchestrator)
- **model**: Identifies neural models (LSTM, MLP, Transformer, etc.)
- **operation**: Identifies operation types (model_inference, http_request, db_query)
- **symbol**: Trading symbols (AAPL, NVDA, MSFT, etc.) - context dependent

### Metric Types Distribution
- **Counters**: 15+ metrics (totals, errors, network bytes)
- **Gauges**: 25+ metrics (usage percentages, counts, scores)
- **Histograms**: 8+ metrics (durations, response times)

## Configuration and Environment

### Docker Production Setup
```yaml
environment:
  - PROMETHEUS_METRICS_ENABLED=true
  - METRICS_PORT=9092
ports:
  - "127.0.0.1:9092:9092"  # Health/Metrics endpoint
```

### Prometheus Scrape Configuration
```yaml
- job_name: 'neural-trader'
  static_configs:
    - targets: ['neural_trader_app:9092']
  metrics_path: '/metrics'
  scrape_interval: 10s
```

## Health Monitoring Integration

The metrics are integrated with a comprehensive health monitoring system that tracks:

1. **Component Registration**: Database, Redis, Neural System, DAA Orchestrator
2. **Health Checks**: Periodic component health assessment
3. **Performance Tracking**: Response times and error rates
4. **System Resources**: CPU, memory, disk, and network monitoring

## Additional Endpoints

Beyond `/metrics`, the health server provides:
- `/health` - Overall system health with JSON response
- `/health/live` - Kubernetes liveness probe
- `/health/ready` - Kubernetes readiness probe with critical component checks

## Implementation Notes

### Metric Registration
- Uses both declarative (`Counter::noop()`) and dynamic (`metrics::counter!()`) registration
- Fallback to simplified counters/gauges when full metrics system unavailable
- Real-time metric updates during application operation

### Performance Considerations
- Metrics collection runs on 60-second intervals (configurable)
- System monitoring uses `sysinfo` for accurate resource measurement
- Metric storage includes automatic cleanup (last 1000 entries for errors)

### Security
- Metrics endpoint only exposed to localhost in production
- No authentication required (typical for metrics endpoints)
- Accessible within Docker network for Prometheus scraping

## Monitoring Recommendations

1. **Alert Thresholds**:
   - `system_health_score < 0.8` (Warning)
   - `component_health_status{component="Database"} < 1` (Critical)
   - `cpu_usage_percent > 80` (Warning)
   - `memory_usage_percent > 85` (Warning)

2. **Key Metrics to Monitor**:
   - Trading performance: `neural_trader_successful_trades_total`
   - Model health: `neural_trader_models_available`
   - System stability: `component_health_status`
   - Resource usage: CPU, memory, disk utilization

3. **Grafana Dashboard Suggestions**:
   - Business metrics dashboard (trading performance, model accuracy)
   - System health dashboard (component status, resource usage)
   - Performance dashboard (response times, error rates)

The neural-trader application provides comprehensive observability through well-structured Prometheus metrics, enabling effective monitoring and alerting for production deployments.