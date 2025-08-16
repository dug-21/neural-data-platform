# Monitoring Guide: Data Backfill System

## Overview

This guide provides comprehensive instructions for monitoring the Data Backfill System in production, including metrics collection, dashboard setup, alerting configuration, and troubleshooting procedures.

## Monitoring Architecture

```
┌─────────────────────────────────────────────────────────┐
│                  Monitoring Stack                        │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌─────────────┐    ┌─────────────┐    ┌────────────┐ │
│  │  Backfill   │───▶│ Prometheus  │───▶│  Grafana   │ │
│  │   System    │    │  Metrics    │    │ Dashboards │ │
│  └─────────────┘    └─────────────┘    └────────────┘ │
│         │                   │                   │        │
│         ▼                   ▼                   ▼        │
│  ┌─────────────┐    ┌─────────────┐    ┌────────────┐ │
│  │   Logs      │───▶│   Loki      │───▶│  Alerts    │ │
│  │ (Structured)│    │             │    │ (PagerDuty)│ │
│  └─────────────┘    └─────────────┘    └────────────┘ │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

## Key Metrics

### System Metrics

#### Download Metrics
```python
# Prometheus metrics exposed by the backfill system

# Counter: Total files downloaded
backfill_downloads_total{status="success|failed", symbol="AAPL"}

# Gauge: Current download speed (MB/s)
backfill_download_speed_mbps{worker_id="1"}

# Histogram: Download duration per file
backfill_download_duration_seconds{bucket="0.5,1,2,5,10,30,60"}

# Counter: Total bytes downloaded
backfill_bytes_downloaded_total{symbol="AAPL"}
```

#### Processing Metrics
```python
# Counter: Records processed
backfill_records_processed_total{symbol="AAPL", status="valid|invalid"}

# Gauge: Current processing rate (records/second)
backfill_processing_rate{worker_id="1"}

# Histogram: Batch processing time
backfill_batch_duration_seconds{size="10000"}

# Counter: Validation failures
backfill_validation_failures_total{type="ohlc|duplicate|gap"}
```

#### Database Metrics
```python
# Counter: Database inserts
backfill_db_inserts_total{table="market_data", status="success|failed"}

# Histogram: Insert batch duration
backfill_db_insert_duration_seconds{batch_size="10000"}

# Gauge: Database connection pool usage
backfill_db_connections{state="active|idle|waiting"}

# Counter: Transaction rollbacks
backfill_db_rollbacks_total{reason="conflict|error"}
```

#### System Resource Metrics
```python
# Gauge: Memory usage
backfill_memory_usage_bytes{component="processor|downloader"}

# Gauge: CPU usage percentage
backfill_cpu_usage_percent{worker_id="1"}

# Gauge: Disk I/O
backfill_disk_io_bytes_per_second{operation="read|write"}

# Gauge: Network bandwidth
backfill_network_bandwidth_mbps{direction="in|out"}
```

## Grafana Dashboards

### Main Dashboard: Backfill Overview

```json
{
  "dashboard": {
    "title": "Data Backfill Overview",
    "panels": [
      {
        "title": "Download Progress",
        "type": "graph",
        "targets": [{
          "expr": "sum(rate(backfill_downloads_total[5m])) by (status)"
        }]
      },
      {
        "title": "Processing Rate",
        "type": "gauge",
        "targets": [{
          "expr": "sum(backfill_processing_rate)"
        }]
      },
      {
        "title": "Error Rate",
        "type": "stat",
        "targets": [{
          "expr": "sum(rate(backfill_downloads_total{status='failed'}[5m])) / sum(rate(backfill_downloads_total[5m])) * 100"
        }]
      }
    ]
  }
}
```

### Performance Dashboard

#### Key Panels

1. **Throughput Panel**
```promql
# Records per second over time
sum(rate(backfill_records_processed_total[1m]))

# Download speed
avg(backfill_download_speed_mbps)

# Database insert rate
sum(rate(backfill_db_inserts_total[1m]))
```

2. **Latency Panel**
```promql
# P50, P95, P99 download times
histogram_quantile(0.50, backfill_download_duration_seconds)
histogram_quantile(0.95, backfill_download_duration_seconds)
histogram_quantile(0.99, backfill_download_duration_seconds)

# Database insert latency
histogram_quantile(0.95, backfill_db_insert_duration_seconds)
```

3. **Resource Usage Panel**
```promql
# Memory usage by component
backfill_memory_usage_bytes / 1024 / 1024 / 1024  # GB

# CPU usage
avg(backfill_cpu_usage_percent) by (worker_id)

# Connection pool utilization
backfill_db_connections{state="active"} / sum(backfill_db_connections) * 100
```

### Progress Tracking Dashboard

```yaml
panels:
  - title: "Files Progress"
    query: |
      (sum(backfill_downloads_total{status="success"}) / 
       (sum(backfill_downloads_total{status="success"}) + 
        count(backfill_pending_files))) * 100
    
  - title: "Estimated Time Remaining"
    query: |
      (count(backfill_pending_files) * 
       avg(backfill_download_duration_seconds)) / 
       count(backfill_active_workers) / 3600  # hours
    
  - title: "Symbol Progress"
    query: |
      count by (symbol) (
        backfill_symbol_complete{status="done"}
      ) / count by (symbol) (
        backfill_symbol_total
      ) * 100
```

## Alerting Configuration

### Critical Alerts

#### 1. Download Failures
```yaml
alert: HighDownloadFailureRate
expr: |
  (sum(rate(backfill_downloads_total{status="failed"}[5m])) / 
   sum(rate(backfill_downloads_total[5m]))) > 0.1
for: 5m
labels:
  severity: critical
annotations:
  summary: "High download failure rate ({{ $value | humanizePercentage }})"
  description: "More than 10% of downloads are failing"
```

#### 2. Processing Stopped
```yaml
alert: ProcessingStopped
expr: sum(backfill_processing_rate) == 0
for: 10m
labels:
  severity: critical
annotations:
  summary: "Backfill processing has stopped"
  description: "No records processed in the last 10 minutes"
```

#### 3. Database Connection Exhaustion
```yaml
alert: DatabaseConnectionPoolExhausted
expr: |
  (backfill_db_connections{state="waiting"} / 
   sum(backfill_db_connections)) > 0.8
for: 5m
labels:
  severity: critical
annotations:
  summary: "Database connection pool near exhaustion"
  description: "{{ $value | humanizePercentage }} connections waiting"
```

### Warning Alerts

#### 1. Slow Processing
```yaml
alert: SlowProcessingRate
expr: sum(backfill_processing_rate) < 5000
for: 15m
labels:
  severity: warning
annotations:
  summary: "Processing rate below target"
  description: "Current rate: {{ $value }} records/sec (target: 10,000)"
```

#### 2. High Memory Usage
```yaml
alert: HighMemoryUsage
expr: backfill_memory_usage_bytes > 1.5 * 1024 * 1024 * 1024  # 1.5GB
for: 10m
labels:
  severity: warning
annotations:
  summary: "High memory usage detected"
  description: "Memory usage: {{ $value | humanize1024 }}B"
```

## Logging Configuration

### Structured Logging Format
```json
{
  "timestamp": "2024-07-24T10:30:45.123Z",
  "level": "INFO",
  "component": "backfill.downloader",
  "event": "download_complete",
  "symbol": "AAPL",
  "date": "2024-07-23",
  "duration_ms": 1234,
  "size_bytes": 52428800,
  "records": 390,
  "correlation_id": "abc123",
  "worker_id": 3
}
```

### Log Queries

#### Find Slow Downloads
```logql
{job="backfill"} 
  | json 
  | event="download_complete" 
  | duration_ms > 30000
  | line_format "{{.symbol}} {{.date}} took {{.duration_ms}}ms"
```

#### Track Errors
```logql
{job="backfill"} 
  | json 
  | level="ERROR"
  | line_format "{{.timestamp}} [{{.component}}] {{.message}}"
```

#### Monitor Progress
```logql
sum by (symbol) (
  count_over_time(
    {job="backfill"} 
    | json 
    | event="file_processed"[5m]
  )
)
```

## Real-time Monitoring Commands

### CLI Monitoring
```bash
# Real-time progress
python -m data_ingestion.backfill monitor --live

# Output:
# ┌─────────────────────────────────────────┐
# │ Backfill Monitor - Live                 │
# ├─────────────────────────────────────────┤
# │ Downloads: 156/523 (29.8%)              │
# │ Processing: 145/156 (92.9%)             │
# │ Speed: 9,234 records/sec                │
# │ Errors: 2 (1.3%)                        │
# │ ETA: 2h 34m                             │
# └─────────────────────────────────────────┘

# Check specific symbol
python -m data_ingestion.backfill status --symbol AAPL
```

### Database Monitoring
```sql
-- Real-time insert rate
SELECT 
    date_trunc('minute', NOW()) as minute,
    COUNT(*) as inserts_per_minute,
    COUNT(DISTINCT symbol) as symbols,
    pg_size_pretty(
        pg_total_relation_size('market_data') - 
        lag(pg_total_relation_size('market_data')) 
        OVER (ORDER BY date_trunc('minute', NOW()))
    ) as size_growth
FROM market_data
WHERE provider = 'polygon_s3'
  AND time > NOW() - INTERVAL '1 minute'
GROUP BY minute;

-- Check for gaps
WITH expected_minutes AS (
    SELECT generate_series(
        date_trunc('day', CURRENT_DATE),
        date_trunc('day', CURRENT_DATE) + INTERVAL '6.5 hours',
        INTERVAL '1 minute'
    ) as expected_time
),
actual_data AS (
    SELECT DISTINCT date_trunc('minute', time) as actual_time
    FROM market_data
    WHERE symbol = 'AAPL'
      AND provider = 'polygon_s3'
      AND date_trunc('day', time) = CURRENT_DATE
)
SELECT COUNT(*) as missing_minutes
FROM expected_minutes e
LEFT JOIN actual_data a ON e.expected_time = a.actual_time
WHERE a.actual_time IS NULL
  AND EXTRACT(hour FROM e.expected_time) BETWEEN 9 AND 16;
```

## Performance Monitoring

### Identify Bottlenecks
```bash
# Profile CPU usage
python -m data_ingestion.backfill profile --cpu

# Profile memory usage
python -m data_ingestion.backfill profile --memory

# Network bandwidth test
python -m data_ingestion.backfill test --bandwidth
```

### Performance Tuning Metrics
```promql
# Identify slowest operations
topk(5, histogram_quantile(0.95, backfill_operation_duration_seconds))

# Find resource constraints
max(backfill_cpu_usage_percent) > 90
max(backfill_memory_usage_bytes) > 1.8 * 1024 * 1024 * 1024

# Check I/O bottlenecks
rate(backfill_disk_io_bytes_per_second[5m]) > 100 * 1024 * 1024
```

## Monitoring Best Practices

### 1. Dashboard Organization
- **Overview**: High-level system health
- **Performance**: Detailed metrics
- **Progress**: Completion tracking
- **Errors**: Error analysis and debugging
- **Resources**: System resource usage

### 2. Alert Fatigue Prevention
- Set appropriate thresholds
- Use alert grouping
- Implement alert suppression during maintenance
- Regular alert review and tuning

### 3. Capacity Planning
```sql
-- Estimate storage needs
SELECT 
    pg_size_pretty(
        AVG(pg_total_relation_size('market_data')) * 
        (5 * 252)  -- 5 years * trading days
    ) as estimated_total_size;

-- Growth rate analysis
SELECT 
    date_trunc('day', time) as day,
    pg_size_pretty(
        COUNT(*) * AVG(pg_column_size(market_data.*))
    ) as daily_size
FROM market_data
WHERE provider = 'polygon_s3'
GROUP BY day
ORDER BY day DESC;
```

### 4. SLA Monitoring
```yaml
# Define SLIs (Service Level Indicators)
- metric: download_success_rate
  target: 99.5%
  query: |
    sum(rate(backfill_downloads_total{status="success"}[5m])) /
    sum(rate(backfill_downloads_total[5m])) * 100

- metric: processing_throughput
  target: 10000  # records/second
  query: sum(backfill_processing_rate)

- metric: data_accuracy
  target: 99.9%
  query: |
    sum(rate(backfill_records_processed_total{status="valid"}[5m])) /
    sum(rate(backfill_records_processed_total[5m])) * 100
```

## Troubleshooting with Monitoring

### Common Issues

#### 1. Sudden Performance Drop
```bash
# Check recent changes
git log --oneline -10

# Review error logs
tail -f /var/log/neural-trader/backfill.log | grep ERROR

# Check system resources
htop
iotop
iftop
```

#### 2. Memory Leaks
```python
# Enable memory profiling
import tracemalloc
tracemalloc.start()

# Monitor memory growth
SELECT 
    pid,
    pg_size_pretty(pg_stat_get_backend_memory_contexts(pid)) as memory
FROM pg_stat_activity
WHERE application_name = 'backfill';
```

#### 3. Network Issues
```bash
# Test S3 connectivity
aws s3 ls s3://flatfiles/us_stocks_sip/minute_aggs_v1/ \
    --endpoint-url https://files.polygon.io

# Check DNS resolution
nslookup files.polygon.io

# Monitor network latency
mtr files.polygon.io
```

## Monitoring Automation

### Automated Reports
```python
# Daily summary report
schedule.every().day.at("09:00").do(generate_daily_report)

def generate_daily_report():
    metrics = {
        'files_processed': get_metric('backfill_downloads_total'),
        'records_inserted': get_metric('backfill_records_processed_total'),
        'errors': get_metric('backfill_errors_total'),
        'avg_speed': get_metric('backfill_processing_rate')
    }
    
    send_email(
        subject="Backfill Daily Report",
        body=render_template('daily_report.html', metrics=metrics)
    )
```

### Health Check Endpoint
```python
@app.route('/health')
def health_check():
    checks = {
        's3_connection': check_s3_connection(),
        'db_connection': check_db_connection(),
        'processing_active': check_processing_active(),
        'error_rate': calculate_error_rate()
    }
    
    status = 'healthy' if all(checks.values()) else 'unhealthy'
    return jsonify({
        'status': status,
        'checks': checks,
        'timestamp': datetime.utcnow().isoformat()
    })
```

---

*Document Version: 1.0.0 | Last Updated: July 2024*