# Dashboard Connection Fix Analysis

## Root Cause of Disconnection

The dashboard metrics are failing to connect due to **port mismatches** between what Prometheus is trying to scrape and what the data-ingestion service is actually exposing.

### Issue Identified

1. **Prometheus Configuration**: The Prometheus config (`docker/production/configs/prometheus/prometheus.yml`) is configured to scrape:
   ```yaml
   - job_name: 'data-ingestion'
     static_configs:
       - targets: ['neural_trader_data_ingestion:8001']  # Scraping port 8001
   ```

2. **Docker Compose Configuration**: The docker-compose.prod.yml maps:
   ```yaml
   ports:
     - "127.0.0.1:8002:8001"  # External port 8002 maps to internal port 8001
   ```

3. **Data Ingestion Service**: The service appears to be configured to serve metrics on port 8001 internally, but Prometheus is trying to scrape from the internal container port.

## Connection Issues Found

### Primary Issue: Metric Naming Convention Mismatch

**Dashboard Expectations vs. Actual Exports:**

The Grafana dashboard (`data_ingestion/monitoring/grafana-dashboard.json`) expects metrics with specific naming patterns:

**Dashboard expects:**
- `up{job="data-ingestion"}`
- `data_ingestion_provider_requests_total`
- `data_ingestion_provider_errors_total`
- `data_ingestion_total_requests`
- `data_ingestion_errors_total`
- `data_ingestion_provider_request_duration_bucket`

**But actual metrics may be using different naming patterns or not being exported correctly.**

### Secondary Issues:

1. **Port Configuration**: Prometheus config targets the correct internal port (8001) but there may be internal connectivity issues
2. **Container Network**: Services are on different networks (`monitoring` vs `neural_trader_internal`)
3. **Health Check Conflicts**: The health check endpoint and metrics endpoint may be conflicting

## Specific Changes Needed

### 1. Fix Prometheus Target Configuration

**File:** `/workspaces/neural-trader/docker/production/configs/prometheus/prometheus.yml`

**Current:**
```yaml
- job_name: 'data-ingestion'
  static_configs:
    - targets: ['neural_trader_data_ingestion:8001']
```

**Should be verified as:**
```yaml
- job_name: 'data-ingestion'
  static_configs:
    - targets: ['neural_trader_data_ingestion:8001']
  metrics_path: '/metrics'
  scrape_interval: 10s
```

### 2. Ensure Data Ingestion Exposes Metrics Correctly

**File:** `/workspaces/neural-trader/data_ingestion/main.py`

The service needs to ensure metrics are being exported on port 8001 at `/metrics` endpoint.

**Current code shows:**
```python
metrics_port = 9090  # Default Prometheus metrics port
if hasattr(self.settings, 'prometheus_port'):
    metrics_port = self.settings.prometheus_port
```

**Fix needed:** Ensure metrics_port is consistently set to 8001 to match container configuration.

### 3. Fix Docker Compose Network Configuration

**File:** `/workspaces/neural-trader/docker/production/docker-compose.prod.yml`

**Current:**
```yaml
data-ingestion:
  networks:
    - neural_trader_internal
    - monitoring
```

This looks correct, but verify the prometheus container can reach data-ingestion.

### 4. Verify Metric Export Names

**File:** `/workspaces/neural-trader/data_ingestion/utils/metrics.py`

Ensure the metrics being exported match exactly what the dashboard expects:
- `data_ingestion_provider_requests_total`
- `data_ingestion_provider_errors_total`
- `data_ingestion_total_requests`
- `data_ingestion_errors_total`

## Testing Approach

### 1. Container Connectivity Test
```bash
# Test if prometheus can reach data-ingestion
docker exec neural_trader_prometheus wget -q --spider http://neural_trader_data_ingestion:8001/metrics
```

### 2. Metrics Export Verification
```bash
# Check what metrics are actually being exported
curl http://localhost:8002/metrics
```

### 3. Prometheus Target Status
```bash
# Check Prometheus target status
curl http://localhost:9093/api/v1/targets
```

### 4. Dashboard Query Test
```bash
# Test specific dashboard queries in Prometheus
curl "http://localhost:9093/api/v1/query?query=up{job=\"data-ingestion\"}"
```

## Implementation Priority

1. **IMMEDIATE**: Fix metrics port consistency in data-ingestion service
2. **HIGH**: Verify metric naming conventions match dashboard expectations
3. **MEDIUM**: Test container network connectivity
4. **LOW**: Optimize scrape intervals and timeouts

## Expected Resolution Time

This is a configuration mismatch issue that should be resolvable within **30-60 minutes** once the proper port and metric naming alignment is implemented.

## Root Cause Summary

The disconnection is caused by a simple **port configuration inconsistency** where the data-ingestion service may not be consistently serving metrics on the expected port 8001, combined with potential **metric naming mismatches** between what the dashboard queries expect and what the service actually exports.