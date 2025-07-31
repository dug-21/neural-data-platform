# Docker Production Deployment Guide

## Overview

This guide provides deployment instructions for the production Docker Compose configuration (`docker-compose.prod.yml`) that includes all dashboard services with resolved port conflicts and infrastructure improvements.

## Key Fixes Applied

### 1. Port Conflict Resolution
- **Neural Trader metrics**: Changed from port 9090 to 9092
- **Prometheus**: Consistent mapping 9090:9090 (was 9091:9090)
- **Data Ingestion**: Added on port 8001

### 2. Missing Exporters Added
- **postgres-exporter**: Port 9187 for TimescaleDB metrics
- **redis-exporter**: Port 9121 for Redis metrics  
- **node-exporter**: Port 9100 for system metrics

### 3. Volume Path Fixes
- **Prometheus config**: `./configs/prometheus/` (was `./monitoring/`)
- **Grafana dashboards**: `./grafana/dashboards/` (was `./monitoring/grafana/dashboards/`)
- **All paths**: Consistent `./configs/` prefix

### 4. Network Consistency
- **Network name**: `neural-trader-network` (was `neural-network`)
- **All services**: Using consistent network naming

### 5. Service Additions
- **data-ingestion**: Complete service definition with health checks
- **All exporters**: Properly configured with correct targets

## Pre-Deployment Requirements

### 1. Environment Variables
Create or update `.env` file in the production directory:

```bash
# Database
POSTGRES_PASSWORD=your_secure_password

# Grafana
GRAFANA_ADMIN_PASSWORD=your_grafana_password

# Market Data (for data-ingestion service)
ALPACA_API_KEY=your_alpaca_api_key
ALPACA_SECRET_KEY=your_alpaca_secret_key
```

### 2. Directory Structure
Ensure these directories exist:

```bash
mkdir -p docker/production/configs/{neural-trader,data-ingestion,prometheus}
mkdir -p docker/production/grafana/{dashboards,datasources}
mkdir -p docker/production/logs/{data-ingestion}
mkdir -p docker/production/init-scripts
```

### 3. Configuration Files
Verify these files exist:
- `configs/prometheus/prometheus.yml` (updated with correct ports)
- `configs/prometheus/alerts.yml`
- `configs/prometheus/neural_prediction_alerts.yml`
- `grafana/datasources/prometheus.yml`
- `grafana/dashboards/` (dashboard JSON files)

## Deployment Steps

### 1. Stop Existing Services
```bash
# Stop any running services
docker-compose -f docker-compose.yml down

# Remove old containers if needed
docker-compose -f docker-compose.yml rm -f
```

### 2. Deploy Fixed Configuration
```bash
# Deploy with fixed configuration
docker-compose -f docker-compose.prod.yml up -d

# Verify all services are starting
docker-compose -f docker-compose.prod.yml ps
```

### 3. Health Check Verification
```bash
# Wait for services to be healthy (may take 1-2 minutes)
docker-compose -f docker-compose.prod.yml logs --tail=50

# Check specific service health
docker-compose -f docker-compose.prod.yml exec neural-trader curl -f http://localhost:8080/health
docker-compose -f docker-compose.prod.yml exec data-ingestion curl -f http://localhost:8001/health
```

### 4. Prometheus Target Verification
```bash
# Check Prometheus targets (all should be UP)
curl -s http://localhost:9090/api/v1/targets | jq '.data.activeTargets[] | {job: .labels.job, health: .health, lastError: .lastError}'

# Expected targets:
# - neural-trader:9092 (UP)
# - data-ingestion:8001 (UP)  
# - postgres-exporter:9187 (UP)
# - redis-exporter:9121 (UP)
# - node-exporter:9100 (UP)
# - localhost:9090 (UP)
```

### 5. Grafana Dashboard Access
```bash
# Access Grafana
echo "Grafana URL: http://localhost:3000"
echo "Username: admin"
echo "Password: ${GRAFANA_ADMIN_PASSWORD}"

# Verify datasource connectivity
curl -s http://admin:${GRAFANA_ADMIN_PASSWORD}@localhost:3000/api/datasources/1 | jq '.url, .access'
```

## Service Port Mapping

| Service | Internal Port | External Port | Purpose |
|---------|--------------|---------------|---------|
| neural-trader | 8080 | 8080 | Main API |
| neural-trader | 9092 | 9092 | Metrics (FIXED) |
| data-ingestion | 8001 | 8001 | Data ingestion API & metrics |
| timescaledb | 5432 | 5432 | PostgreSQL database |
| redis | 6379 | 6379 | Redis cache |
| prometheus | 9090 | 9090 | Prometheus UI (FIXED) |
| grafana | 3000 | 3000 | Grafana dashboards |
| postgres-exporter | 9187 | 9187 | PostgreSQL metrics |
| redis-exporter | 9121 | 9121 | Redis metrics |
| node-exporter | 9100 | 9100 | System metrics |

## Metrics Endpoints Verification

Test all metrics endpoints are accessible:

```bash
# Neural Trader metrics (FIXED port)
curl http://localhost:9092/metrics | head -20

# Data Ingestion metrics
curl http://localhost:8001/metrics | head -20

# PostgreSQL metrics
curl http://localhost:9187/metrics | head -20

# Redis metrics  
curl http://localhost:9121/metrics | head -20

# System metrics
curl http://localhost:9100/metrics | head -20

# Prometheus metrics
curl http://localhost:9090/metrics | head -20
```

## Troubleshooting

### Common Issues

#### 1. Port Conflicts
If you see "port already in use" errors:
```bash
# Check what's using the port
sudo lsof -i :9090  # or other conflicting port

# Stop conflicting service or change port in compose file
```

#### 2. Missing Metrics Data
If Prometheus shows targets as DOWN:
```bash
# Check service logs
docker-compose -f docker-compose.prod.yml logs [service-name]

# Test internal connectivity  
docker-compose -f docker-compose.prod.yml exec prometheus ping neural-trader
```

#### 3. Volume Mount Issues
If config files aren't found:
```bash
# Verify files exist
ls -la configs/prometheus/
ls -la grafana/dashboards/

# Check container mounts
docker-compose -f docker-compose.prod.yml exec prometheus ls -la /etc/prometheus/
```

### Log Analysis
```bash
# View all service logs
docker-compose -f docker-compose.prod.yml logs

# Follow specific service logs
docker-compose -f docker-compose.prod.yml logs -f neural-trader
docker-compose -f docker-compose.prod.yml logs -f prometheus
```

## Validation Checklist

- [ ] All services start successfully (`docker-compose ps` shows healthy)
- [ ] Prometheus targets all show as UP (`http://localhost:9090/targets`)
- [ ] Neural Trader metrics accessible on port 9092
- [ ] Data Ingestion service responds on port 8001
- [ ] All exporters provide metrics data
- [ ] Grafana can connect to Prometheus datasource
- [ ] Dashboards display data correctly
- [ ] No port conflicts with existing services

## Rollback Plan

If issues occur, rollback to previous configuration:
```bash
# Stop fixed configuration
docker-compose -f docker-compose.prod.yml down

# Start previous configuration  
docker-compose -f docker-compose.yml up -d
```

## Next Steps

After successful deployment:
1. Import/verify Grafana dashboards
2. Configure alerting rules in Prometheus
3. Set up monitoring alerts
4. Performance test with load
5. Configure backup procedures

## Support

For issues with this deployment, check:
1. Service logs: `docker-compose -f docker-compose.prod.yml logs [service]`
2. Network connectivity: `docker network inspect neural-trader-network`
3. Health checks: `docker-compose -f docker-compose.prod.yml ps`
4. SPARC completion document: `products/features/dashboard1/plan/sparc-completion.md`