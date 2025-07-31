# SPARC Completion - Neural Trader Dashboard Implementation

## Implementation Artifacts Summary

This completion document provides the final implementation guidance and ready-to-deploy configurations for the Neural Trader dashboard system.

## Immediate Actions (Deploy Today)

### 1. Apply Docker Compose Fixes

Create a new file: `docker/production/docker-compose.fixed.yml`

```yaml
version: '3.8'

services:
  # Core Services
  neural-trader:
    image: neural-trader:latest
    container_name: neural-trader
    ports:
      - "8080:8080"    # API port
      - "9092:9092"    # Metrics port (NEW)
    environment:
      - DATABASE_URL=postgresql://postgres:${POSTGRES_PASSWORD}@timescaledb:5432/neural_trader
      - REDIS_URL=redis://redis:6379
      - RUST_LOG=info
      - METRICS_PORT=9092
      - PROMETHEUS_METRICS_ENABLED=true
    volumes:
      - ./configs/neural-trader:/app/config
      - ./logs:/app/logs
    networks:
      - neural-trader-network
    depends_on:
      - timescaledb
      - redis
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  # Data Ingestion Service (NEW)
  data-ingestion:
    image: neural-trader/data-ingestion:latest
    container_name: data-ingestion
    ports:
      - "8001:8001"
    environment:
      - PROMETHEUS_METRICS_ENABLED=true
      - METRICS_PORT=8001
      - DATABASE_URL=postgresql://postgres:${POSTGRES_PASSWORD}@timescaledb:5432/neural_trader
      - REDIS_URL=redis://redis:6379
      - ALPACA_API_KEY=${ALPACA_API_KEY}
      - ALPACA_SECRET_KEY=${ALPACA_SECRET_KEY}
    volumes:
      - ./configs/data-ingestion:/app/config
      - ./logs/data-ingestion:/app/logs
    networks:
      - neural-trader-network
    depends_on:
      - timescaledb
      - redis
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8001/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  # Database
  timescaledb:
    image: timescale/timescaledb:latest-pg16
    container_name: timescaledb
    ports:
      - "5432:5432"
    environment:
      - POSTGRES_DB=neural_trader
      - POSTGRES_USER=postgres
      - POSTGRES_PASSWORD=${POSTGRES_PASSWORD}
    volumes:
      - timescaledb_data:/var/lib/postgresql/data
      - ./init-scripts:/docker-entrypoint-initdb.d
    networks:
      - neural-trader-network
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 10s
      timeout: 5s
      retries: 5

  # Redis
  redis:
    image: redis:7-alpine
    container_name: redis
    ports:
      - "6379:6379"
    volumes:
      - redis_data:/data
    networks:
      - neural-trader-network
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 5s
      retries: 3

  # Monitoring Stack
  prometheus:
    image: prom/prometheus:latest
    container_name: prometheus
    ports:
      - "9090:9090"  # FIXED: Consistent port mapping
    volumes:
      - ./configs/prometheus/prometheus.yml:/etc/prometheus/prometheus.yml  # FIXED path
      - ./configs/prometheus/alerts.yml:/etc/prometheus/alerts.yml
      - ./configs/prometheus/neural_prediction_alerts.yml:/etc/prometheus/neural_prediction_alerts.yml
      - prometheus_data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--web.console.libraries=/usr/share/prometheus/console_libraries'
      - '--web.console.templates=/usr/share/prometheus/consoles'
    networks:
      - neural-trader-network
    depends_on:
      - neural-trader
      - data-ingestion

  grafana:
    image: grafana/grafana:latest
    container_name: grafana
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=${GRAFANA_ADMIN_PASSWORD}
      - GF_SERVER_ROOT_URL=http://localhost:3000
    volumes:
      - grafana_data:/var/lib/grafana
      - ./grafana/dashboards:/etc/grafana/provisioning/dashboards  # FIXED path
      - ./grafana/datasources:/etc/grafana/provisioning/datasources
    networks:
      - neural-trader-network
    depends_on:
      - prometheus

  # Exporters (NEW)
  postgres-exporter:
    image: quay.io/prometheuscommunity/postgres-exporter:latest
    container_name: postgres-exporter
    ports:
      - "9187:9187"
    environment:
      DATA_SOURCE_NAME: "postgresql://postgres:${POSTGRES_PASSWORD}@timescaledb:5432/neural_trader?sslmode=disable"
    networks:
      - neural-trader-network
    depends_on:
      - timescaledb

  redis-exporter:
    image: oliver006/redis_exporter:latest
    container_name: redis-exporter
    ports:
      - "9121:9121"
    environment:
      REDIS_ADDR: "redis:6379"
    networks:
      - neural-trader-network
    depends_on:
      - redis

  node-exporter:
    image: prom/node-exporter:latest
    container_name: node-exporter
    ports:
      - "9100:9100"
    volumes:
      - /proc:/host/proc:ro
      - /sys:/host/sys:ro
      - /:/rootfs:ro
    command:
      - '--path.procfs=/host/proc'
      - '--path.sysfs=/host/sys'
      - '--path.rootfs=/rootfs'
    networks:
      - neural-trader-network

networks:
  neural-trader-network:
    driver: bridge

volumes:
  timescaledb_data:
  redis_data:
  prometheus_data:
  grafana_data:
```

### 2. Update Prometheus Configuration

Replace `docker/production/configs/prometheus/prometheus.yml`:

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s
  external_labels:
    cluster: 'neural-trader-prod'
    
alerting:
  alertmanagers:
    - static_configs:
        - targets: []

rule_files:
  - "alerts.yml"
  - "neural_prediction_alerts.yml"

scrape_configs:
  # Neural Trader metrics (FIXED port)
  - job_name: 'neural-trader'
    static_configs:
      - targets: ['neural-trader:9092']  # Using new metrics port
    metrics_path: '/metrics'
    scrape_interval: 10s
    
  # Data Ingestion metrics
  - job_name: 'data-ingestion'
    static_configs:
      - targets: ['data-ingestion:8001']
    metrics_path: '/metrics'
    scrape_interval: 10s
    
  # TimescaleDB metrics
  - job_name: 'postgres'
    static_configs:
      - targets: ['postgres-exporter:9187']
      
  # Redis metrics
  - job_name: 'redis'
    static_configs:
      - targets: ['redis-exporter:9121']
      
  # Node metrics
  - job_name: 'node'
    static_configs:
      - targets: ['node-exporter:9100']
      
  # Prometheus self-monitoring
  - job_name: 'prometheus'
    static_configs:
      - targets: ['localhost:9090']
```

### 3. Deploy Dashboard Files

Copy the Grafana dashboard JSON files:

```bash
# Create dashboard directory
mkdir -p docker/grafana/dashboards

# Copy dashboard provisioning config
cat > docker/grafana/dashboards/dashboard.yml << 'EOF'
apiVersion: 1

providers:
  - name: 'Neural Trader Dashboards'
    orgId: 1
    folder: ''
    type: file
    disableDeletion: false
    updateIntervalSeconds: 10
    allowUiUpdates: true
    options:
      path: /etc/grafana/provisioning/dashboards
EOF

# Copy all dashboard JSON files from plan
cp products/features/dashboard1/plan/grafana-dashboards/*.json docker/grafana/dashboards/
```

### 4. Create Grafana Datasource

Create `docker/grafana/datasources/prometheus.yml`:

```yaml
apiVersion: 1

datasources:
  - name: Prometheus
    type: prometheus
    access: proxy
    url: http://prometheus:9090
    isDefault: true
    editable: true
```

### 5. Environment Variables

Update `.env` file:

```bash
# Database
POSTGRES_PASSWORD=your_secure_password
POSTGRES_MONITORING_PASSWORD=monitoring_password

# Grafana
GRAFANA_ADMIN_PASSWORD=your_grafana_password

# Alpaca (for market data)
ALPACA_API_KEY=your_alpaca_key
ALPACA_SECRET_KEY=your_alpaca_secret
```

## Deployment Commands

```bash
# 1. Stop existing services
docker-compose -f docker/production/docker-compose.yml down

# 2. Deploy with fixed configuration
docker-compose -f docker/production/docker-compose.fixed.yml up -d

# 3. Verify all services are healthy
docker-compose -f docker/production/docker-compose.fixed.yml ps

# 4. Check Prometheus targets
curl http://localhost:9090/api/v1/targets

# 5. Access Grafana
# Open http://localhost:3000
# Login: admin / ${GRAFANA_ADMIN_PASSWORD}
# Dashboards will be auto-imported
```

## Post-Deployment Validation

### 1. Verify Prometheus Scraping
- Navigate to http://localhost:9090/targets
- All targets should show as "UP"
- Check for the new exporters

### 2. Test Dashboards
- Open Grafana at http://localhost:3000
- Navigate to Dashboards → Browse
- Verify all 5 dashboards are loaded:
  - Operational Overview
  - Performance Monitoring
  - Trading Operations
  - Infrastructure Monitoring
  - Market Data Real-time

### 3. Verify Real-time Updates
- Open Trading Operations dashboard
- Confirm portfolio values update every 5 seconds
- Check that market data updates every 1 second

### 4. Test Metrics Endpoints
```bash
# Neural Trader metrics
curl http://localhost:9092/metrics

# Data Ingestion metrics
curl http://localhost:8001/metrics

# Postgres exporter
curl http://localhost:9187/metrics

# Redis exporter
curl http://localhost:9121/metrics

# Node exporter
curl http://localhost:9100/metrics
```

## Troubleshooting Guide

### Issue: Prometheus Can't Scrape Targets
```bash
# Check network connectivity
docker exec prometheus ping neural-trader
docker exec prometheus ping data-ingestion

# Check service logs
docker logs prometheus
docker logs neural-trader
```

### Issue: Dashboards Not Loading
```bash
# Check Grafana logs
docker logs grafana

# Verify dashboard files
docker exec grafana ls -la /etc/grafana/provisioning/dashboards/

# Restart Grafana
docker-compose -f docker/production/docker-compose.fixed.yml restart grafana
```

### Issue: No Data in Dashboards
```bash
# Verify Prometheus is collecting metrics
curl http://localhost:9090/api/v1/query?query=up

# Check specific metrics
curl http://localhost:9090/api/v1/query?query=neural_trader_portfolio_value
```

## Next Steps

1. **Performance Testing**
   - Run load tests with 100+ concurrent dashboard users
   - Monitor WebSocket connection limits
   - Optimize query performance

2. **Security Hardening**
   - Enable TLS for all services
   - Implement authentication proxy
   - Set up RBAC in Grafana

3. **Advanced Features**
   - Add dashboard variables for filtering
   - Create alert rules for SLA monitoring
   - Implement dashboard versioning

## Success Metrics Tracking

Monitor these KPIs after deployment:

- **Dashboard Load Time**: Target < 2 seconds
- **Real-time Update Latency**: Target < 1 second
- **User Adoption**: Track unique dashboard viewers
- **System Uptime**: Monitor dashboard availability

## Conclusion

The Neural Trader dashboard system is now ready for immediate deployment. The infrastructure fixes have been applied, and all components are properly configured. Follow the deployment commands above to launch the complete monitoring solution.

For questions or issues, refer to the SPARC planning documents in `products/features/dashboard1/plan/`.