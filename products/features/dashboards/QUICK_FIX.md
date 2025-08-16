# Quick Fix for Grafana Dashboard Connectivity

## The Simple Fix

**Issue**: Health check uses HTTPS instead of HTTP

**File**: `/workspaces/neural-trader/docker/production/docker-compose.prod.yml`  
**Line**: 101

### Change This:
```yaml
healthcheck:
  test: ["CMD", "curl", "-f", "https://localhost:9092/health"]
```

### To This:
```yaml
healthcheck:
  test: ["CMD", "curl", "-f", "http://localhost:9092/health"]
```

## Why This Fixes It

1. The health server runs HTTP not HTTPS
2. Failed health checks may affect service discovery
3. Prometheus scraping may be affected by unhealthy service status

## Test the Fix

```bash
# 1. Apply the fix and restart
docker-compose -f docker/production/docker-compose.prod.yml down
docker-compose -f docker/production/docker-compose.prod.yml up -d

# 2. Verify health endpoint works
curl http://localhost:9092/health

# 3. Check Prometheus can scrape
curl http://localhost:9092/metrics

# 4. Verify Prometheus targets
curl http://localhost:9093/api/v1/targets
```

## Expected Results

After the fix:
- Health checks should pass
- Prometheus should successfully scrape basic health metrics
- Grafana should show at least the "Neural Trader Status" panel with data

## Additional Notes

The dashboards expect more metrics than currently implemented. This fix will restore basic connectivity, but full dashboard functionality requires implementing additional Prometheus metrics in the Rust application.

Current available metrics:
- `system_health_score`
- `component_health_status` 
- `healthy_components_total`
- `health_server_uptime_seconds`

Missing expected metrics:
- `up{job="neural-trader"}`
- `trades_executed_total`
- `total_pnl`
- `http_request_duration_seconds_bucket`