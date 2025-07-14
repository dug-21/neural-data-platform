# Prometheus Queries for Data Ingestion Monitoring

## API Performance Metrics

### Request Rate by Provider
```promql
sum(rate(data_ingestion_api_requests_total[5m])) by (provider)
```

### API Error Rate
```promql
sum(rate(data_ingestion_api_requests_total{status="error"}[5m])) by (provider) 
/ 
sum(rate(data_ingestion_api_requests_total[5m])) by (provider)
```

### API Request Duration (95th percentile)
```promql
histogram_quantile(0.95, 
  sum(rate(data_ingestion_api_request_duration_seconds_bucket[5m])) by (provider, le)
)
```

## Data Processing Metrics

### Data Points Processing Rate
```promql
sum(rate(data_ingestion_points_processed_total[5m])) by (provider, data_type)
```

### Processing Error Rate
```promql
sum(rate(data_ingestion_processing_errors_total[5m])) by (provider, error_type)
```

### Data Quality Score
```promql
data_ingestion_provider_data_quality_score
```

## Pipeline Performance

### Pipeline Stage Duration (99th percentile)
```promql
histogram_quantile(0.99,
  sum(rate(data_ingestion_pipeline_stage_duration_seconds_bucket[5m])) by (pipeline, stage, le)
)
```

### Pipeline Throughput
```promql
data_ingestion_pipeline_throughput_items_per_second
```

### Pipeline Backpressure
```promql
data_ingestion_pipeline_backpressure > 0.5
```

## Storage Metrics

### Database Write Throughput
```promql
sum(rate(data_ingestion_db_write_throughput_rows_per_second[5m])) by (table)
```

### Database Write Batch Size Distribution
```promql
histogram_quantile(0.5,
  sum(rate(data_ingestion_db_write_batch_size_bucket[5m])) by (table, le)
)
```

### Redis Publish Rate
```promql
sum(rate(data_ingestion_redis_publish_total[5m])) by (channel_type)
```

### Redis Publish Message Size (95th percentile)
```promql
histogram_quantile(0.95,
  sum(rate(data_ingestion_redis_publish_size_bytes_bucket[5m])) by (channel_type, le)
)
```

## System Health

### Active Connections
```promql
data_ingestion_active_connections
```

### Provider Health Score
```promql
data_ingestion_provider_health_score < 0.8
```

### Concurrent Tasks
```promql
data_ingestion_concurrent_tasks
```

### Rate Limit Hits
```promql
sum(rate(data_ingestion_rate_limit_hits_total[5m])) by (provider)
```

## Scheduler Metrics

### Scheduler Job Duration
```promql
histogram_quantile(0.95,
  sum(rate(data_ingestion_scheduler_run_duration_seconds_bucket[5m])) by (scheduler_type, job_name, le)
)
```

### Scheduler Lag
```promql
data_ingestion_scheduler_lag_seconds > 60
```

### Batch Job Success Rate
```promql
sum(rate(data_ingestion_batch_job_success_total[5m])) by (job_id)
/
(sum(rate(data_ingestion_batch_job_success_total[5m])) by (job_id) + 
 sum(rate(data_ingestion_batch_job_errors_total[5m])) by (job_id))
```

## Alerting Rules

### High Error Rate Alert
```yaml
alert: HighAPIErrorRate
expr: |
  sum(rate(data_ingestion_api_requests_total{status="error"}[5m])) by (provider) 
  / 
  sum(rate(data_ingestion_api_requests_total[5m])) by (provider) > 0.1
for: 5m
labels:
  severity: warning
annotations:
  summary: "High API error rate for {{ $labels.provider }}"
  description: "Error rate is {{ $value | humanizePercentage }} for provider {{ $labels.provider }}"
```

### Provider Health Alert
```yaml
alert: ProviderUnhealthy
expr: data_ingestion_provider_health_score < 0.5
for: 10m
labels:
  severity: critical
annotations:
  summary: "Provider {{ $labels.provider }} is unhealthy"
  description: "Health score is {{ $value }} for provider {{ $labels.provider }}"
```

### Pipeline Backpressure Alert
```yaml
alert: HighPipelineBackpressure
expr: data_ingestion_pipeline_backpressure > 0.8
for: 5m
labels:
  severity: warning
annotations:
  summary: "High backpressure in {{ $labels.pipeline }} pipeline"
  description: "Backpressure is {{ $value }} in stage {{ $labels.stage }}"
```

### Database Connection Pool Alert
```yaml
alert: DatabaseConnectionPoolExhausted
expr: |
  data_ingestion_db_connection_pool_size{state="idle"} 
  / 
  data_ingestion_db_connection_pool_size{state="total"} < 0.1
for: 5m
labels:
  severity: critical
annotations:
  summary: "Database connection pool nearly exhausted"
  description: "Only {{ $value | humanizePercentage }} of connections are idle"
```

## Grafana Dashboard Panels

### Row 1: Overview
1. **Total Request Rate** - Single stat
2. **Overall Error Rate** - Single stat with threshold
3. **Active Providers** - Single stat
4. **System Health Score** - Gauge

### Row 2: API Performance
1. **Request Rate by Provider** - Time series
2. **API Latency Distribution** - Heatmap
3. **Error Rate by Provider** - Time series
4. **Rate Limit Hits** - Time series

### Row 3: Data Processing
1. **Data Points Processing Rate** - Time series
2. **Processing Errors by Type** - Stacked area
3. **Data Quality Scores** - Table
4. **Validation Failure Rate** - Time series

### Row 4: Pipeline Performance
1. **Pipeline Stage Duration** - Heatmap
2. **Pipeline Throughput** - Time series
3. **Pipeline Backpressure** - Time series
4. **Concurrent Tasks** - Time series

### Row 5: Storage
1. **DB Write Throughput** - Time series
2. **DB Write Batch Sizes** - Histogram
3. **Redis Publish Rate** - Time series
4. **Storage Operation Latency** - Heatmap

### Row 6: System Resources
1. **Active Connections** - Stacked area
2. **Connection Pool Usage** - Gauge
3. **Queue Sizes** - Time series
4. **Memory Usage** - Time series