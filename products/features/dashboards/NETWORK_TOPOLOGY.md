# Neural Trader Network Topology Analysis

## Executive Summary

Analysis of Docker networking and service connectivity in the neural-trader production environment reveals several critical issues affecting metrics collection and monitoring capabilities.

### Key Findings

1. **PORT MISMATCH**: Critical discrepancy in neural-trader metrics port configuration
2. **SERVICE DISCOVERY**: Container naming inconsistencies affecting Prometheus scraping
3. **NETWORK ISOLATION**: Mixed network configurations causing connectivity issues
4. **HEALTHCHECK ERROR**: Invalid HTTPS protocol in health check configuration

## Service Topology Map

### Core Services Network Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      NEURAL TRADER PRODUCTION NETWORK                  │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐     │
│  │   TimescaleDB   │    │      Redis      │    │  Neural Trader  │     │
│  │                 │    │                 │    │                 │     │
│  │ Container Name: │    │ Container Name: │    │ Container Name: │     │
│  │neural_trader_   │    │neural_trader_   │    │neural_trader_   │     │
│  │timescaledb      │    │redis            │    │app              │     │
│  │                 │    │                 │    │                 │     │
│  │ Hostname:       │    │ Hostname:       │    │ Hostname:       │     │
│  │ timescaledb     │    │ redis           │    │ neural-trader   │     │
│  │                 │    │                 │    │                 │     │
│  │ Internal Port:  │    │ Internal Port:  │    │ Internal Ports: │     │
│  │ 5432            │    │ 6379            │    │ 8080 (API)      │     │
│  │                 │    │                 │    │ 9092 (Health)   │     │
│  │ External Port:  │    │ No external     │    │                 │     │
│  │ 5433            │    │ access          │    │ External Ports: │     │
│  └─────────────────┘    └─────────────────┘    │ 8080, 9092      │     │
│                                                └─────────────────┘     │
│                                                                         │
│  ┌─────────────────┐                                                    │
│  │ Data Ingestion  │                                                    │
│  │                 │                                                    │
│  │ Container Name: │                                                    │
│  │neural_trader_   │                                                    │
│  │data_ingestion   │                                                    │
│  │                 │                                                    │
│  │ Hostname:       │                                                    │
│  │ data-ingestion  │                                                    │
│  │                 │                                                    │
│  │ Internal Port:  │                                                    │
│  │ 8001            │                                                    │
│  │                 │                                                    │
│  │ External Port:  │                                                    │
│  │ 8002            │                                                    │
│  └─────────────────┘                                                    │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│                         MONITORING NETWORK                             │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐     │
│  │   Prometheus    │    │     Grafana     │    │ Postgres Export │     │
│  │                 │    │                 │    │                 │     │
│  │ Container Name: │    │ Container Name: │    │ Container Name: │     │
│  │neural_trader_   │    │neural_trader_   │    │ postgres-       │     │
│  │prometheus       │    │grafana          │    │ exporter        │     │
│  │                 │    │                 │    │                 │     │
│  │ Hostname:       │    │ Hostname:       │    │ Internal Port:  │     │
│  │ prometheus      │    │ grafana         │    │ 9187 (default)  │     │
│  │                 │    │                 │    │                 │     │
│  │ Internal Port:  │    │ Internal Port:  │    │ No external     │     │
│  │ 9090            │    │ 3000            │    │ access          │     │
│  │                 │    │                 │    │                 │     │
│  │ External Port:  │    │ External Port:  │    │ ┌─────────────┐ │     │
│  │ 9093            │    │ 3000            │    │ │Redis Export │ │     │
│  └─────────────────┘    └─────────────────┘    │ │             │ │     │
│                                                │ │redis-       │ │     │
│  ┌─────────────────┐                          │ │exporter     │ │     │
│  │ Node Exporter   │                          │ │             │ │     │
│  │                 │                          │ │Port: 9121   │ │     │
│  │ Container Name: │                          │ │(internal)   │ │     │
│  │ node-exporter   │                          │ └─────────────┘ │     │
│  │                 │                          └─────────────────┘     │
│  │ Internal Port:  │                                                    │
│  │ 9100 (default)  │                                                    │
│  │                 │                                                    │
│  │ No external     │                                                    │
│  │ access          │                                                    │
│  └─────────────────┘                                                    │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## Network Configuration Analysis

### Network Definitions

```yaml
networks:
  neural_trader_internal:
    driver: bridge
    # Note: internal: true removed to allow external API access
    
  monitoring:
    driver: bridge
```

### Service Network Memberships

| Service | neural_trader_internal | monitoring | External Access |
|---------|------------------------|------------|----------------|
| timescaledb | ✅ | ❌ | 127.0.0.1:5433 |
| redis | ✅ | ❌ | ❌ |
| neural-trader | ✅ | ✅ | 127.0.0.1:8080,9092 |
| data-ingestion | ✅ | ✅ | 127.0.0.1:8002 |
| prometheus | ❌ | ✅ | 127.0.0.1:9093 |
| grafana | ✅ | ✅ | 127.0.0.1:3000 |
| postgres-exporter | ✅ | ✅ | ❌ |
| redis-exporter | ✅ | ✅ | ❌ |
| node-exporter | ❌ | ✅ | ❌ |

## Critical Issues Identified

### 🚨 Issue #1: Container Name vs Hostname Mismatch

**Problem**: Prometheus configuration uses container names, but services may not be discoverable

**Evidence**:
```yaml
# Prometheus scrape config
- targets: ['neural_trader_app:9092']        # Container name
- targets: ['neural_trader_data_ingestion:8001']  # Container name

# But service hostnames are different
hostname: neural-trader      # Not neural_trader_app
hostname: data-ingestion     # Not neural_trader_data_ingestion
```

**Impact**: Prometheus cannot discover services, leading to "connection refused" errors

**Resolution**: Use hostname instead of container_name in Prometheus targets:
```yaml
- targets: ['neural-trader:9092']
- targets: ['data-ingestion:8001']
```

### 🚨 Issue #2: Health Check Protocol Error

**Problem**: Health check uses HTTPS on internal HTTP endpoint

**Evidence**:
```yaml
healthcheck:
  test: ["CMD", "curl", "-f", "https://localhost:9092/health"]  # HTTPS ❌
```

**Impact**: Health checks fail, containers marked as unhealthy

**Resolution**: Change to HTTP:
```yaml
healthcheck:
  test: ["CMD", "curl", "-f", "http://localhost:9092/health"]   # HTTP ✅
```

### 🚨 Issue #3: Data Ingestion Port Mismatch

**Problem**: Prometheus tries to scrape port 8001, but external port is 8002

**Evidence**:
```yaml
# Prometheus config
- targets: ['neural_trader_data_ingestion:8001']

# Docker compose
ports:
  - "127.0.0.1:8002:8001"  # External:Internal mapping
```

**Impact**: Prometheus scraping fails for data ingestion metrics

**Resolution**: Internal scraping should use port 8001 (correct), but ensure network connectivity

### ⚠️  Issue #4: Network Segmentation Problems

**Problem**: Mixed network access causing potential connectivity issues

**Evidence**:
- Prometheus only on `monitoring` network
- Neural-trader on both networks
- TimescaleDB only on `neural_trader_internal` network

**Impact**: Complex routing requirements, potential access issues

**Resolution**: Verify all monitoring components can reach target services

## Service Discovery Mechanism Analysis

### Current Configuration

The system uses **static configuration** for service discovery in Prometheus:

```yaml
scrape_configs:
  - job_name: 'neural-trader'
    static_configs:
      - targets: ['neural_trader_app:9092']  # ❌ Wrong hostname
        
  - job_name: 'data-ingestion'  
    static_configs:
      - targets: ['neural_trader_data_ingestion:8001']  # ❌ Wrong hostname
```

### Recommended Configuration

```yaml
scrape_configs:
  - job_name: 'neural-trader'
    static_configs:
      - targets: ['neural-trader:9092']  # ✅ Correct hostname
        
  - job_name: 'data-ingestion'
    static_configs:
      - targets: ['data-ingestion:8001']  # ✅ Correct hostname
```

## Port Mapping Analysis

### Neural Trader Service

| Purpose | Internal Port | External Port | Protocol | Status |
|---------|---------------|---------------|----------|--------|
| API/MCP | 8080 | 8080 | HTTP | ✅ Working |
| Health/Metrics | 9092 | 9092 | HTTP | ⚠️ See issues |

**Configuration**:
- Environment: `METRICS_PORT=9092` ✅
- Code: Health server runs on port 9092 ✅
- Docker ports: `"127.0.0.1:9092:9092"` ✅

### Data Ingestion Service

| Purpose | Internal Port | External Port | Protocol | Status |
|---------|---------------|---------------|----------|--------|
| API | 8001 | 8002 | HTTP | ✅ Working |

### Monitoring Services

| Service | Internal Port | External Port | Purpose |
|---------|---------------|---------------|---------|
| Prometheus | 9090 | 9093 | Web UI & API |
| Grafana | 3000 | 3000 | Dashboard UI |
| Postgres Exporter | 9187 | - | Metrics only |
| Redis Exporter | 9121 | - | Metrics only |
| Node Exporter | 9100 | - | Metrics only |

## Grafana Datasource Configuration Analysis

### Current Datasource Configuration

```yaml
datasources:
  - name: Prometheus
    type: prometheus
    access: proxy
    url: http://prometheus:9090  # ✅ Correct internal hostname
    isDefault: true
    
  - name: TimescaleDB
    type: postgres
    url: timescaledb:5432        # ✅ Correct internal hostname
    database: neural_trader
```

**Status**: ✅ **CORRECT** - Uses proper service hostnames

### Network Access Verification

Grafana can access:
- ✅ Prometheus: Both on `monitoring` network
- ✅ TimescaleDB: Grafana has access to `neural_trader_internal` network

## Metrics Endpoint Status

### Neural Trader Metrics

**Endpoint**: `http://neural-trader:9092/metrics`

**Available Metrics**:
```prometheus
# System Health
system_health_score
component_health_status{component="database|redis|neural_system|daa_orchestrator"}
healthy_components_total
unhealthy_components_total
health_server_uptime_seconds

# Performance
component_health_check_duration_seconds
component_response_time (histogram)
```

### Data Ingestion Metrics

**Endpoint**: `http://data-ingestion:8001/metrics`

**Status**: ⚠️ **NEEDS VERIFICATION** - Check if metrics endpoint exists

### Exporter Metrics

**Available Endpoints**:
- `http://postgres-exporter:9187/metrics` ✅
- `http://redis-exporter:9121/metrics` ✅  
- `http://node-exporter:9100/metrics` ✅

## Recommended Fixes

### Priority 1 (Critical)

1. **Fix Prometheus targets** in `/docker/production/configs/prometheus/prometheus.yml`:
```yaml
scrape_configs:
  - job_name: 'neural-trader'
    static_configs:
      - targets: ['neural-trader:9092']  # Change from neural_trader_app
        
  - job_name: 'data-ingestion'
    static_configs:
      - targets: ['data-ingestion:8001']  # Change from neural_trader_data_ingestion
```

2. **Fix health check protocol** in `docker-compose.prod.yml`:
```yaml
healthcheck:
  test: ["CMD", "curl", "-f", "http://localhost:9092/health"]  # HTTP not HTTPS
```

### Priority 2 (Important)

3. **Verify data ingestion metrics**: Check if `/metrics` endpoint exists
4. **Add explicit network connectivity tests** in health checks
5. **Standardize container naming** vs hostname usage

### Priority 3 (Enhancement)

6. **Add service discovery labels** for better monitoring
7. **Implement metrics endpoint documentation**
8. **Add network connectivity monitoring**

## Testing & Validation

### Manual Verification Commands

```bash
# Test service connectivity from prometheus container
docker exec neural_trader_prometheus curl -f http://neural-trader:9092/metrics
docker exec neural_trader_prometheus curl -f http://data-ingestion:8001/metrics

# Test health endpoints
curl http://localhost:9092/health
curl http://localhost:9092/metrics

# Verify Prometheus targets
curl http://localhost:9093/api/v1/targets | jq '.data.activeTargets'
```

### Expected Results

After fixes, Prometheus targets should show:
```json
{
  "discoveredLabels": {
    "job": "neural-trader"
  },
  "labels": {
    "instance": "neural-trader:9092",
    "job": "neural-trader"
  },
  "scrapeUrl": "http://neural-trader:9092/metrics",
  "health": "up"
}
```

## Architecture Recommendations

### Short Term (1-2 days)
- Fix Prometheus target hostnames
- Fix health check protocol
- Verify all metric endpoints are accessible

### Medium Term (1 week)
- Implement comprehensive connectivity monitoring
- Add service discovery automation
- Standardize port allocation strategy

### Long Term (1 month)
- Migrate to DNS-based service discovery
- Implement distributed tracing
- Add network performance monitoring

## Conclusion

The neural-trader Docker network topology has a solid foundation but suffers from critical service discovery issues. The primary problems are hostname mismatches in Prometheus configuration and protocol errors in health checks. These issues prevent proper metrics collection and monitoring.

With the recommended fixes, the system should achieve full observability with Prometheus successfully scraping metrics from all services and Grafana displaying comprehensive dashboards.