# PromQL Queries for Data Ingestion Monitoring

## Service Health Queries

### Basic Health Check
```promql
# Service up/down status
up{job="data-ingestion"}

# Provider health status
up{job="data-ingestion-provider"} by (provider)
```

## Throughput Metrics

### Overall Throughput
```promql
# Total requests per second
sum(rate(data_ingestion_total_requests[5m]))

# Requests per second by provider
sum by (provider) (rate(data_ingestion_provider_requests_total[5m]))

# Throughput trend over time
sum(rate(data_ingestion_total_requests[5m])) - sum(rate(data_ingestion_total_requests[5m] offset 1h))
```

## Error Rate Calculations

### Overall Error Rate
```promql
# Error rate percentage
(sum(rate(data_ingestion_errors_total[5m])) / sum(rate(data_ingestion_total_requests[5m]))) * 100

# Success rate percentage
(1 - (sum(rate(data_ingestion_errors_total[5m])) / sum(rate(data_ingestion_total_requests[5m])))) * 100
```

### Provider-Specific Error Rates
```promql
# Error rate by provider
(sum by (provider) (rate(data_ingestion_provider_errors_total[5m])) 
 / sum by (provider) (rate(data_ingestion_provider_requests_total[5m]))) * 100

# Providers with high error rates (>10%)
(sum by (provider) (rate(data_ingestion_provider_errors_total[5m])) 
 / sum by (provider) (rate(data_ingestion_provider_requests_total[5m]))) > 0.10
```

### Error Type Analysis
```promql
# Errors by type
sum by (error_type) (rate(data_ingestion_errors_by_type_total[5m]))

# Top 5 error types
topk(5, sum by (error_type) (rate(data_ingestion_errors_by_type_total[5m])))
```

## Latency Analysis

### Percentile Calculations
```promql
# 50th percentile (median) latency
histogram_quantile(0.50, sum(rate(data_ingestion_request_duration_bucket[5m])) by (le))

# 95th percentile latency
histogram_quantile(0.95, sum(rate(data_ingestion_request_duration_bucket[5m])) by (le))

# 99th percentile latency
histogram_quantile(0.99, sum(rate(data_ingestion_request_duration_bucket[5m])) by (le))
```

### Provider Latency Comparison
```promql
# Provider p95 latencies
histogram_quantile(0.95, sum by (provider, le) (rate(data_ingestion_provider_request_duration_bucket[5m])))

# Slowest providers (p95 > 2s)
histogram_quantile(0.95, sum by (provider, le) (rate(data_ingestion_provider_request_duration_bucket[5m]))) > 2
```

## Pipeline Performance

### Stage Throughput
```promql
# Throughput by pipeline stage
sum by (stage) (rate(data_ingestion_pipeline_stage_total[5m]))

# Stage processing rate comparison
sum by (stage) (rate(data_ingestion_pipeline_stage_total[5m])) 
/ ignoring(stage) group_left sum(rate(data_ingestion_total_requests[5m]))
```

### Stage Latencies
```promql
# p99 latency by stage
histogram_quantile(0.99, sum by (stage, le) (rate(data_ingestion_pipeline_stage_duration_bucket[5m])))

# Slowest pipeline stages
topk(3, histogram_quantile(0.99, sum by (stage, le) (rate(data_ingestion_pipeline_stage_duration_bucket[5m]))))
```

## Data Freshness Monitoring

### Symbol Data Age
```promql
# Time since last update per symbol
time() - data_ingestion_last_update_timestamp

# Symbols not updated in last 5 minutes
(time() - data_ingestion_last_update_timestamp) > 300

# Average data age across all symbols
avg(time() - data_ingestion_last_update_timestamp)
```

### Freshness Metrics
```promql
# Percentage of fresh data
(count(data_ingestion_symbol_data_fresh{fresh="true"}) / count(data_ingestion_active_symbols)) * 100

# Count of stale symbols
count((time() - data_ingestion_last_update_timestamp) > 300)

# Symbols with critically stale data (>15 min)
count((time() - data_ingestion_last_update_timestamp) > 900)
```

## Resource Usage

### Memory Metrics
```promql
# Memory usage in GB
process_resident_memory_bytes{job="data-ingestion"} / 1024 / 1024 / 1024

# Memory usage percentage (assuming 8GB limit)
(process_resident_memory_bytes{job="data-ingestion"} / (8 * 1024 * 1024 * 1024)) * 100
```

### CPU Metrics
```promql
# CPU usage rate
rate(process_cpu_seconds_total{job="data-ingestion"}[5m]) * 100

# CPU cores used
rate(process_cpu_seconds_total{job="data-ingestion"}[5m])
```

### Connection Pool
```promql
# Available connections
data_ingestion_connection_pool_available

# Connection pool utilization
(1 - (data_ingestion_connection_pool_available / data_ingestion_connection_pool_size)) * 100

# Connection wait time
histogram_quantile(0.95, rate(data_ingestion_connection_wait_duration_bucket[5m]))
```

## Advanced Analytics

### Trend Analysis
```promql
# Request rate change over last hour
(sum(rate(data_ingestion_total_requests[5m])) - sum(rate(data_ingestion_total_requests[5m] offset 1h))) 
/ sum(rate(data_ingestion_total_requests[5m] offset 1h)) * 100

# Error rate trend
data_ingestion:error_rate:5m - data_ingestion:error_rate:5m offset 1h
```

### Capacity Planning
```promql
# Request rate growth prediction (linear regression over 1 day)
predict_linear(sum(rate(data_ingestion_total_requests[5m]))[1d:], 3600)

# Memory usage prediction for next hour
predict_linear(process_resident_memory_bytes{job="data-ingestion"}[1h:], 3600)
```

### SLI/SLO Calculations
```promql
# Availability SLI (last 24h)
avg_over_time(up{job="data-ingestion"}[24h]) * 100

# Latency SLI (% of requests under 1s)
sum(rate(data_ingestion_request_duration_bucket{le="1"}[24h])) 
/ sum(rate(data_ingestion_request_duration_count[24h])) * 100

# Error rate SLI (last 24h)
1 - (sum(increase(data_ingestion_errors_total[24h])) 
     / sum(increase(data_ingestion_total_requests[24h])))
```

### Comparative Analysis
```promql
# Provider performance ranking by success rate
sort_desc(
  1 - (sum by (provider) (rate(data_ingestion_provider_errors_total[1h])) 
       / sum by (provider) (rate(data_ingestion_provider_requests_total[1h])))
)

# Hour-over-hour comparison
sum(rate(data_ingestion_total_requests[5m])) 
/ sum(rate(data_ingestion_total_requests[5m] offset 1h))
```

## Alerting Queries

### Critical Alerts
```promql
# Service completely down
up{job="data-ingestion"} == 0

# No data processing
sum(rate(data_ingestion_total_requests[5m])) == 0

# Extreme error rate (>25%)
(sum(rate(data_ingestion_errors_total[5m])) / sum(rate(data_ingestion_total_requests[5m]))) > 0.25
```

### Warning Alerts
```promql
# Degraded performance (p99 > 5s)
histogram_quantile(0.99, sum(rate(data_ingestion_request_duration_bucket[5m])) by (le)) > 5

# Low data freshness (<80%)
(count(data_ingestion_symbol_data_fresh{fresh="true"}) / count(data_ingestion_active_symbols)) < 0.80

# High memory usage (>80% of limit)
(process_resident_memory_bytes{job="data-ingestion"} / (8 * 1024 * 1024 * 1024)) > 0.80
```

## Dashboard Variables

### Useful Prometheus Variables
```promql
# Get all providers
label_values(data_ingestion_provider_requests_total, provider)

# Get all error types
label_values(data_ingestion_errors_by_type_total, error_type)

# Get all pipeline stages
label_values(data_ingestion_pipeline_stage_total, stage)

# Get all active symbols
label_values(data_ingestion_last_update_timestamp, symbol)
```