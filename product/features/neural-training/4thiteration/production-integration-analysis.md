# Production Docker Compose Integration Analysis

## Executive Summary

The neural-trader project has **TWO production Docker Compose files** with different configurations and philosophies. Both already include Prometheus and Grafana, but with significant differences that need to be reconciled for proper integration.

## Key Findings

### 1. Two Production Configurations

#### Root Level: `/workspaces/neural-trader/docker-compose.prod.yml`
- **Size**: 289 lines, comprehensive production setup
- **Architecture**: Uses Docker Swarm mode with overlay networks
- **Monitoring**: Prometheus on port 9090, Grafana on port 3000
- **Security**: Uses Docker secrets for all sensitive data
- **Scale**: Configured for multi-replica deployments
- **Networks**: Three separate networks (frontend, backend, monitoring)

#### Production Directory: `/workspaces/neural-trader/docker/production/docker-compose.prod.yml`
- **Size**: 209 lines, cleaner setup
- **Architecture**: Standard Docker Compose with bridge networks
- **Monitoring**: Prometheus on port 9093 (to avoid conflicts), Grafana on port 3000
- **Security**: Environment variables for secrets
- **Scale**: Single instance deployments
- **Networks**: Two networks (neural_trader_internal, monitoring)

### 2. Critical Port Conflicts Discovered

**Data Ingestion Service Ports:**
- **Health Check Port**: 8001 (API endpoint)
- **Metrics Port**: 9090 (Prometheus metrics)

**Current Configurations:**
- Root `docker-compose.prod.yml`: Health check uses port 8001
- Production directory: Maps 8002:8001 (API) and 9092:9090 (metrics)
- Prometheus scrapes: data-ingestion:9090/metrics

### 3. Monitoring Stack Analysis

#### Root Level Configuration
```yaml
prometheus:
  ports: ["9090:9090"]
  networks: [neural_trader_monitoring, neural_trader_backend]
  
grafana:
  ports: ["3000:3000"]
  networks: [neural_trader_monitoring, neural_trader_frontend]
```

#### Production Directory Configuration
```yaml
prometheus:
  ports: ["127.0.0.1:9093:9090"]  # Avoids conflict with VSCode
  networks: [monitoring]
  
grafana:
  ports: ["127.0.0.1:3000:3000"]
  networks: [monitoring, neural_trader_internal]
```

## Integration Recommendations

### 1. Determine Primary Configuration

**Recommendation**: Use the root level `docker-compose.prod.yml` as the primary production configuration because:
- It's designed for Docker Swarm (production-ready)
- Uses proper secrets management
- Has multi-replica support
- More comprehensive service definitions

### 2. Port Standardization

```yaml
# Standardized port mapping
data-ingestion:
  ports:
    - "8001:8001"  # API/Health endpoint
    - "9091:9090"  # Metrics endpoint (avoid conflict with Prometheus)
  healthcheck:
    test: ["CMD", "curl", "-f", "http://localhost:8001/health"]
```

### 3. Prometheus Scrape Configuration

Update `/workspaces/neural-trader/docker/prometheus/prometheus.yml`:

```yaml
scrape_configs:
  # Data Ingestion service with correct ports
  - job_name: 'data-ingestion'
    static_configs:
      - targets: ['data-ingestion:9090']  # Internal container port
    metrics_path: '/metrics'
    scrape_interval: 10s
    
  # Neural Trader (when metrics are implemented)
  - job_name: 'neural-trader'
    static_configs:
      - targets: ['neural-trader:3030']  # Assuming metrics on main port
    metrics_path: '/metrics'
```

### 4. Network Architecture

**Recommended Network Setup:**
```yaml
networks:
  neural_trader_frontend:
    driver: overlay
    attachable: true
    
  neural_trader_backend:
    driver: overlay
    internal: true  # No external access
    
  neural_trader_monitoring:
    driver: overlay
    internal: true  # Monitoring isolated
```

### 5. Service Integration Points

#### For Neural Training Enhancement Integration:

1. **Metrics Exposure**:
   - Neural Trader: Expose metrics on port 3030/metrics
   - Data Ingestion: Already exposing on port 9090/metrics
   - Add custom neural training metrics to both services

2. **Grafana Dashboards**:
   - Location: `/docker/grafana/dashboards/`
   - Add: `neural-training-dashboard.json`
   - Add: `model-performance-dashboard.json`

3. **Prometheus Rules**:
   - Location: `/docker/prometheus/alerts/`
   - Add: `neural-training-alerts.yml`
   - Monitor: Training duration, accuracy, resource usage

### 6. Migration Path

1. **Phase 1**: Standardize on root `docker-compose.prod.yml`
2. **Phase 2**: Migrate useful features from `/docker/production/` version
3. **Phase 3**: Add neural training specific services
4. **Phase 4**: Implement comprehensive monitoring

## Specific Integration Steps

### Step 1: Fix Port Conflicts

```yaml
# In docker-compose.prod.yml
data-ingestion:
  ports:
    - "8001:8001"  # API
    - "9091:9090"  # Metrics (external:internal)
```

### Step 2: Add Neural Training Metrics

```yaml
# New service in docker-compose.prod.yml
neural-metrics-collector:
  image: neural-trader/metrics-collector:latest
  environment:
    - PROMETHEUS_PUSHGATEWAY=prometheus:9091
  depends_on:
    - prometheus
  networks:
    - neural_trader_monitoring
```

### Step 3: Enhance Monitoring

```yaml
# Add to existing Grafana service
grafana:
  volumes:
    - ./docker/grafana/dashboards:/etc/grafana/provisioning/dashboards:ro
    - ./docker/grafana/dashboards/neural:/etc/grafana/provisioning/dashboards/neural:ro
```

## Conclusion

The production environment already has a robust monitoring stack with Prometheus and Grafana. The main challenges are:

1. **Configuration Duplication**: Two production configs need consolidation
2. **Port Conflicts**: Data ingestion service port mappings need standardization
3. **Network Isolation**: Current setup is good but needs consistent application
4. **Metrics Integration**: Services need to expose neural training specific metrics

The recommended approach is to enhance the existing root level production configuration rather than creating a third configuration, ensuring compatibility with the established monitoring infrastructure.

## Next Steps

1. Consolidate production configurations
2. Standardize service ports
3. Update Prometheus scrape configurations
4. Add neural training specific dashboards
5. Implement training metrics in application code
6. Test integration in staging environment