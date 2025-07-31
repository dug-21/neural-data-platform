# Fix: Prometheus Scraping Port Configuration

## Issue
Prometheus was unable to scrape metrics from data-ingestion service:
```
Error scraping target: Get "http://data-ingestion:9090/metrics": dial tcp 172.19.0.3:9090: connect: connection refused
```

## Root Cause
Port mismatch between:
1. **Health check server**: Running on port 8001 (serves `/metrics` endpoint)
2. **Dockerfile**: Exposing port 9090
3. **Prometheus config**: Trying to scrape port 9090

## Solution Applied

### 1. Updated Prometheus Configuration
Changed the scrape target from port 9090 to 8001 in:
- `/docker/prometheus/prometheus.yml`
- `/docker/production/configs/prometheus/prometheus.yml` (production config)
```yaml
- job_name: 'data-ingestion'
  static_configs:
    - targets: ['data-ingestion:8001']  # was 9090
  metrics_path: '/metrics'
```

### 2. Updated Dockerfile
Changed the exposed port in `/data_ingestion/Dockerfile`:
```dockerfile
# Expose health check and metrics port
EXPOSE 8001  # was 9090
```

## Port Configuration Summary

| Service | Port | Purpose |
|---------|------|---------|
| Health Check Server | 8001 | Serves `/health`, `/health/detailed`, and `/metrics` endpoints |
| Prometheus Metrics | 8001 | Available at `http://data-ingestion:8001/metrics` |

## Testing

After rebuilding and restarting:

```bash
# Rebuild data-ingestion
docker-compose build data-ingestion

# Restart services
docker-compose up -d data-ingestion prometheus

# Check if Prometheus can scrape
curl http://localhost:9090/targets  # Check Prometheus UI
```

The data-ingestion target should now show as "UP" in Prometheus.

## Note
The metrics are served by the health check server on port 8001, not on a separate metrics port. This consolidates all monitoring endpoints on a single port.