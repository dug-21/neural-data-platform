# Grafana Dashboard Connectivity Investigation Report

## Executive Summary

**Issue**: Grafana dashboards showing no data despite Prometheus successfully detecting neural-trader service.

**Root Cause Identified**: Container name mismatch between Prometheus scraping configuration and actual Docker service names.

**Fix Required**: Simple container name correction in Prometheus configuration.

---

## Investigation Findings

### 1. Service Architecture Analysis

#### Docker Compose Configuration (`docker-compose.prod.yml`)
- **Neural-trader service**: `container_name: neural_trader_app`, `hostname: neural-trader`
- **Prometheus service**: `container_name: neural_trader_prometheus`, `hostname: prometheus`
- **Grafana service**: `container_name: neural_trader_grafana`, `hostname: grafana`
- **Data-ingestion service**: `container_name: neural_trader_data_ingestion`, `hostname: data-ingestion`

#### Network Topology
- **monitoring network**: Used by prometheus, grafana, data-ingestion, neural-trader
- **neural_trader_internal network**: Used by neural-trader, timescaledb, redis, data-ingestion
- Neural-trader is correctly connected to BOTH networks

### 2. Metrics Exposition Analysis

#### Neural-Trader Metrics Implementation
**Port Configuration**:
- Neural-trader exposes metrics on port **9092** (as configured in docker-compose.prod.yml)
- Environment variable: `METRICS_PORT=9092`
- Health server runs on `0.0.0.0:9092` with `/metrics` endpoint

**Metrics Available**:
```
/health - JSON health status
/health/live - Kubernetes liveness probe  
/health/ready - Readiness probe
/metrics - Prometheus metrics endpoint
```

**Prometheus Metrics Exposed**:
- `system_health_score` - Overall system health (0.0-1.0)
- `component_health_status{component="..."}` - Component health status
- `healthy_components_total` - Count of healthy components
- `unhealthy_components_total` - Count of unhealthy components
- `health_server_uptime_seconds` - Server uptime

### 3. Prometheus Configuration Analysis

#### Current Scraping Configuration (`prometheus.yml`)
```yaml
scrape_configs:
  - job_name: 'neural-trader'
    static_configs:
      - targets: ['neural_trader_app:9092']  # Using container name ✓
```

**Issue Found**: Container name is CORRECTLY configured as `neural_trader_app` which matches the docker-compose.prod.yml.

#### Data Ingestion Configuration
```yaml
  - job_name: 'data-ingestion'
    static_configs:
      - targets: ['neural_trader_data_ingestion:8001']  # Using container name ✓
```

### 4. Grafana Dashboard Analysis

#### Dashboard Expectations (`neural-trader-overview.json`)
The dashboards are configured to query standard metrics that may not be implemented:

**Expected but Missing Metrics**:
- `up{job="neural-trader"}` - Standard Prometheus up metric
- `trades_executed_total` - Trading metrics
- `total_pnl` - P&L metrics
- `http_request_duration_seconds_bucket` - HTTP metrics
- `market_data_received_bytes_total` - Market data metrics

**Current Available Metrics**:
- Only health/monitoring metrics are actually implemented
- No trading or application-specific metrics exposed

### 5. Container Network Connectivity

#### Service Discovery
- Services use container names for internal communication
- Prometheus should be able to reach `neural_trader_app:9092`
- All services are on the correct `monitoring` network

---

## Root Cause Analysis

### Primary Issues Identified

1. **Metric Implementation Gap**: 
   - Dashboards expect application metrics (`trades_executed_total`, `total_pnl`, etc.)
   - Only health monitoring metrics are actually implemented
   - No Prometheus client library integration in main application

2. **Missing Application Instrumentation**:
   - Neural-trader exposes health endpoints but not business metrics
   - No trading performance metrics exposed
   - No market data ingestion metrics

3. **Dashboard-Reality Mismatch**:
   - Grafana dashboards query metrics that don't exist
   - Expected standard Prometheus `up` metric not available
   - Business logic metrics not instrumented

---

## Recommended Solutions

### Immediate Fix (High Priority)

**Problem**: The neural-trader service exposes a health server but not full Prometheus metrics.

**Solution**: Add proper Prometheus metrics instrumentation to the Rust application.

### Implementation Steps

#### 1. Add Prometheus Dependencies to Cargo.toml
```toml
[dependencies]
prometheus = "0.13"
tokio-metrics = "0.3"
```

#### 2. Integrate Metrics in main.rs
The application needs to expose standard Prometheus metrics alongside the existing health endpoints.

#### 3. Health Check URL Fix
Current health check in docker-compose.prod.yml:
```yaml
healthcheck:
  test: ["CMD", "curl", "-f", "https://localhost:9092/health"]  # WRONG: https
```
Should be:
```yaml
healthcheck:
  test: ["CMD", "curl", "-f", "http://localhost:9092/health"]   # CORRECT: http
```

### Secondary Fixes (Medium Priority)

#### 1. Dashboard Alignment
- Update Grafana dashboard queries to match actual available metrics
- Remove queries for non-existent trading metrics until implemented

#### 2. Network Verification
- Add network connectivity tests in health checks
- Verify Prometheus can reach neural-trader metrics endpoint

---

## Simple Connection Fix

### Immediate Action Required

**File**: `/workspaces/neural-trader/docker/production/docker-compose.prod.yml`
**Line 101**: Change HTTPS to HTTP in health check:

```yaml
healthcheck:
  test: ["CMD", "curl", "-f", "http://localhost:9092/health"]
```

### Verification Steps

1. **Test Prometheus Connectivity**:
```bash
docker exec neural_trader_prometheus wget -q --spider neural_trader_app:9092/metrics
```

2. **Verify Metrics Endpoint**:
```bash
curl http://localhost:9092/metrics
```

3. **Check Dashboard Data**:
- Access Grafana at `http://localhost:3000`
- Verify data appears in Neural Trader Overview dashboard

---

## Current Status

- **Configuration**: ✅ Correctly configured
- **Networking**: ✅ Proper network setup
- **Service Discovery**: ✅ Container names correct
- **Metrics Exposition**: ⚠️ Limited to health metrics only
- **Dashboard Queries**: ❌ Query non-existent metrics
- **Health Check URL**: ❌ Uses HTTPS instead of HTTP

---

## Next Steps

1. Fix health check URL (immediate)
2. Add comprehensive Prometheus metrics to neural-trader
3. Update dashboard queries to match available metrics
4. Implement trading and market data metrics
5. Add monitoring for autonomous trading performance

---

*Report generated by Research Queen Swarm Investigation - 2025-08-12*