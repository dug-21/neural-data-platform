# Neural Trader Dashboard Deployment Runbook

## 📋 Overview

This runbook provides comprehensive step-by-step instructions for deploying the Neural Trader dashboard system with complete monitoring, observability, and production-ready configurations.

## 🏗️ Architecture Overview

The Neural Trader production deployment consists of:

### Core Services
- **Neural Trader Application** (Port 8080) - Main trading engine with neural predictions
- **Data Ingestion Service** (Port 8001) - Real-time market data processing
- **TimescaleDB** (Port 5432) - Time-series database for market data
- **Redis** (Port 6379) - Caching and real-time data storage

### Monitoring & Observability Stack
- **Prometheus** (Port 9090) - Metrics collection and alerting
- **Grafana** (Port 3000) - Dashboard visualization and monitoring
- **Node Exporter** (Port 9100) - System metrics
- **Postgres Exporter** (Port 9187) - Database metrics
- **Redis Exporter** (Port 9121) - Cache metrics

### Dashboard Components
- **Operational Overview** - System health and performance metrics
- **Trading Operations** - Trading decisions and neural predictions
- **Market Data Realtime** - Live market data and processing metrics  
- **Infrastructure Monitoring** - Resource utilization and system health
- **Performance Monitoring** - Latency, throughput, and optimization metrics

## 🚨 Pre-Deployment Checklist

### 1. System Requirements Verification

```bash
# Check Docker and Docker Compose versions
docker --version                    # Minimum: 20.10+
docker-compose --version           # Minimum: 2.0+

# Verify system resources
free -h                           # Minimum: 8GB RAM
df -h                            # Minimum: 50GB disk space
nproc                           # Minimum: 4 CPU cores
```

### 2. Network Ports Availability

```bash
# Check if required ports are available
netstat -tuln | grep -E ':(3000|5432|6379|8001|8080|9090|9092|9100|9121|9187)'

# If any ports are occupied, either:
# - Stop conflicting services
# - Modify port mappings in docker-compose.yml
```

### 3. Directory Structure Setup

```bash
# Navigate to production directory
cd /path/to/neural-trader/docker/production

# Verify critical directories exist
ls -la configs/
ls -la grafana/dashboards/
ls -la grafana/datasources/

# Create required data directories
sudo mkdir -p /opt/neural-trader/data/{models,backup,exports,timescale,redis}
sudo mkdir -p /opt/neural-trader/logs
sudo chown -R $USER:$USER /opt/neural-trader/
```

### 4. Environment Configuration

```bash
# Copy and customize environment file
cp .env.example .env

# CRITICAL: Set these required variables in .env
cat << 'EOF' >> .env
# Database
POSTGRES_PASSWORD=your_secure_postgres_password_here
DB_PASSWORD=your_secure_postgres_password_here

# Grafana
GRAFANA_ADMIN_PASSWORD=your_secure_grafana_password_here
GRAFANA_PASSWORD=your_secure_grafana_password_here

# Trading APIs (obtain from respective providers)
ALPACA_API_KEY=your_alpaca_api_key
ALPACA_API_SECRET=your_alpaca_api_secret

# Optional: Additional providers
FINNHUB_API_KEY=your_finnhub_api_key
ALPHA_VANTAGE_API_KEY=your_alpha_vantage_api_key
POLYGON_API_KEY=your_polygon_api_key

# Backup (if using S3)
BACKUP_S3_BUCKET=your_backup_bucket
AWS_ACCESS_KEY_ID=your_aws_key
AWS_SECRET_ACCESS_KEY=your_aws_secret
EOF

# Validate environment file
source .env
if [[ -z "$POSTGRES_PASSWORD" ]] || [[ -z "$GRAFANA_ADMIN_PASSWORD" ]]; then
    echo "❌ CRITICAL: Missing required environment variables!"
    exit 1
fi
```

## 🚀 Deployment Steps

### Phase 1: Infrastructure Services

#### Step 1.1: Deploy Database and Cache Services

```bash
# Start infrastructure services first
docker-compose up -d timescaledb redis

# Wait for services to be ready
echo "⏳ Waiting for TimescaleDB to be ready..."
timeout 60 bash -c 'until docker-compose logs timescaledb | grep "database system is ready"; do sleep 5; done'

echo "⏳ Waiting for Redis to be ready..."
timeout 30 bash -c 'until docker-compose logs redis | grep "Ready to accept connections"; do sleep 2; done'

# Verify health status
docker-compose ps
```

#### Step 1.2: Initialize Database Schema

```bash
# Check if init scripts were executed
docker-compose logs timescaledb | grep -i "CREATE"

# If manual initialization needed:
docker-compose exec timescaledb psql -U postgres -d neural_trader -c "\dt"

# Verify TimescaleDB extensions
docker-compose exec timescaledb psql -U postgres -d neural_trader -c "SELECT * FROM pg_extension WHERE extname = 'timescaledb';"
```

#### **✅ Validation Checkpoint 1**

```bash
# Database connectivity test
docker-compose exec timescaledb pg_isready -U postgres -d neural_trader

# Redis connectivity test  
docker-compose exec redis redis-cli ping

# Expected outputs:
# TimescaleDB: "postgres:5432 - accepting connections"
# Redis: "PONG"
```

### Phase 2: Core Application Services

#### Step 2.1: Deploy Neural Trader Application

```bash
# Build application image if needed
docker-compose build neural-trader

# Start the neural trader service
docker-compose up -d neural-trader

# Monitor startup logs
docker-compose logs -f neural-trader

# Wait for application startup (look for "Server started on 0.0.0.0:8080")
timeout 120 bash -c 'until docker-compose logs neural-trader | grep "Server started"; do sleep 5; done'
```

#### Step 2.2: Deploy Data Ingestion Service

```bash
# Start data ingestion service
docker-compose up -d data-ingestion

# Monitor startup logs
docker-compose logs -f data-ingestion

# Wait for service readiness
timeout 60 bash -c 'until curl -f http://localhost:8001/health; do sleep 5; done'
```

#### **✅ Validation Checkpoint 2**

```bash
# Test core application endpoints
curl -f http://localhost:8080/health | jq '.'
curl -f http://localhost:8001/health | jq '.'

# Test metrics endpoints
curl -f http://localhost:8080/metrics | head -20
curl -f http://localhost:8001/metrics | head -20

# Verify neural prediction functionality
curl -f http://localhost:8080/api/predictions/AAPL | jq '.'

# Expected: HTTP 200 responses with proper JSON data
```

### Phase 3: Monitoring and Observability Stack

#### Step 3.1: Deploy Metrics Exporters

```bash
# Start all exporters
docker-compose up -d postgres-exporter redis-exporter node-exporter

# Verify exporter health
curl -f http://localhost:9187/metrics | grep postgres
curl -f http://localhost:9121/metrics | grep redis  
curl -f http://localhost:9100/metrics | grep node
```

#### Step 3.2: Deploy Prometheus

```bash
# Start Prometheus
docker-compose up -d prometheus

# Wait for Prometheus to be ready
echo "⏳ Waiting for Prometheus startup..."
timeout 60 bash -c 'until curl -f http://localhost:9090/-/ready; do sleep 5; done'

# Verify Prometheus targets
echo "📊 Checking Prometheus targets..."
curl -s http://localhost:9090/api/v1/targets | jq '.data.activeTargets[] | {job: .labels.job, health: .health}'
```

#### Step 3.3: Deploy Grafana Dashboards

```bash
# Start Grafana
docker-compose up -d grafana

# Wait for Grafana to be ready
echo "⏳ Waiting for Grafana startup..."
timeout 120 bash -c 'until curl -f http://localhost:3000/api/health; do sleep 5; done'

# Login and verify dashboards are loaded
echo "🎯 Grafana ready at: http://localhost:3000"
echo "   Username: admin"  
echo "   Password: ${GRAFANA_ADMIN_PASSWORD}"
```

#### **✅ Validation Checkpoint 3**

```bash
# Verify Prometheus is scraping all targets
TARGETS_UP=$(curl -s http://localhost:9090/api/v1/targets | jq '.data.activeTargets[] | select(.health=="up") | .labels.job' | wc -l)
echo "✅ Prometheus targets UP: $TARGETS_UP/7 expected"

# Test Grafana API
curl -u admin:${GRAFANA_ADMIN_PASSWORD} http://localhost:3000/api/dashboards/home

# Verify dashboards are accessible
curl -u admin:${GRAFANA_ADMIN_PASSWORD} http://localhost:3000/api/search | jq '.[].title'
```

### Phase 4: Dashboard Access Validation

#### Step 4.1: Operational Overview Dashboard

```bash
# Access the main operational dashboard
open http://localhost:3000/d/operational-overview

# Key metrics to verify:
# - System uptime > 0
# - Memory usage < 80%
# - CPU usage displaying
# - Database connections > 0
# - Redis hit rate > 0%
```

#### Step 4.2: Trading Operations Dashboard

```bash
# Access trading dashboard
open http://localhost:3000/d/trading-operations

# Key metrics to verify:
# - Neural predictions being generated
# - Trading decisions logged
# - P&L calculations
# - Position tracking
# - Risk metrics
```

#### Step 4.3: Market Data Dashboard

```bash
# Access market data dashboard  
open http://localhost:3000/d/market-data-realtime

# Key metrics to verify:
# - Real-time price updates
# - Data ingestion rates
# - Market data latency
# - Symbol coverage
# - Data quality metrics
```

#### Step 4.4: Infrastructure Monitoring

```bash
# Access infrastructure dashboard
open http://localhost:3000/d/infrastructure-monitoring  

# Key metrics to verify:
# - Container health status
# - Resource utilization
# - Network I/O
# - Disk usage
# - Service response times
```

#### **✅ Validation Checkpoint 4**

```bash
# Automated dashboard validation
DASHBOARD_COUNT=$(curl -u admin:${GRAFANA_ADMIN_PASSWORD} -s http://localhost:3000/api/search | jq '. | length')
echo "✅ Grafana dashboards loaded: $DASHBOARD_COUNT"

# Test each dashboard endpoint
for dashboard in operational-overview trading-operations market-data-realtime infrastructure-monitoring performance-monitoring; do
    HTTP_CODE=$(curl -u admin:${GRAFANA_ADMIN_PASSWORD} -s -o /dev/null -w "%{http_code}" http://localhost:3000/d/$dashboard)
    echo "📊 Dashboard $dashboard: HTTP $HTTP_CODE"
done
```

## 🔧 Post-Deployment Configuration

### 1. Grafana Initial Setup

```bash
# Configure Grafana settings
curl -u admin:${GRAFANA_ADMIN_PASSWORD} -X PUT http://localhost:3000/api/org/preferences \
  -H "Content-Type: application/json" \
  -d '{
    "timezone": "UTC",
    "homeDashboardId": 1,
    "theme": "dark"
  }'

# Create additional users (optional)
curl -u admin:${GRAFANA_ADMIN_PASSWORD} -X POST http://localhost:3000/api/admin/users \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Trader",
    "email": "trader@company.com", 
    "login": "trader",
    "password": "secure_password_here"
  }'
```

### 2. Alert Configuration

```bash
# Test Prometheus alerting
curl -s http://localhost:9090/api/v1/rules | jq '.data.groups[].rules[] | select(.type=="alerting") | .name'

# Configure Grafana alert notifications
curl -u admin:${GRAFANA_ADMIN_PASSWORD} -X POST http://localhost:3000/api/alert-notifications \
  -H "Content-Type: application/json" \
  -d '{
    "name": "slack-alerts",
    "type": "slack",
    "settings": {
      "url": "YOUR_SLACK_WEBHOOK_URL",
      "channel": "#trading-alerts"
    }
  }'
```

### 3. Data Retention Policies

```bash
# Configure TimescaleDB retention
docker-compose exec timescaledb psql -U postgres -d neural_trader -c "
  SELECT add_retention_policy('market_data', INTERVAL '90 days');
  SELECT add_retention_policy('neural_predictions', INTERVAL '180 days');
  SELECT add_retention_policy('trading_decisions', INTERVAL '365 days');
"

# Configure Prometheus retention (already set in docker-compose.yml)
echo "✅ Prometheus retention: 200 hours configured"
```

## 🔄 Service Health Monitoring

### Automated Health Checks

```bash
#!/bin/bash
# health_check.sh - Run this script periodically

echo "🔍 Neural Trader Health Check - $(date)"

# Check all container status
echo "📊 Container Status:"
docker-compose ps --format "table {{.Name}}\t{{.State}}\t{{.Status}}"

# Test application endpoints
echo -e "\n🔗 Application Health:"
endpoints=(
  "neural-trader:8080/health"
  "data-ingestion:8001/health" 
  "prometheus:9090/-/ready"
  "grafana:3000/api/health"
)

for endpoint in "${endpoints[@]}"; do
  service=$(echo $endpoint | cut -d: -f1)
  url="http://localhost:$(echo $endpoint | cut -d: -f2-)"
  
  if curl -f -s "$url" > /dev/null; then
    echo "✅ $service: Healthy"
  else
    echo "❌ $service: Unhealthy - Check logs: docker-compose logs $service"
  fi
done

# Check metrics availability
echo -e "\n📈 Metrics Validation:"
metrics_count=$(curl -s http://localhost:9090/api/v1/label/__name__/values | jq '.data | length')
echo "✅ Prometheus metrics available: $metrics_count"

# Check database connectivity
echo -e "\n🗄️ Database Health:"
if docker-compose exec -T timescaledb pg_isready -U postgres -q; then
  echo "✅ TimescaleDB: Connected"
else
  echo "❌ TimescaleDB: Connection failed"
fi

# Check Redis connectivity  
echo -e "\n💾 Cache Health:"
if docker-compose exec -T redis redis-cli ping | grep -q PONG; then
  echo "✅ Redis: Connected"
else
  echo "❌ Redis: Connection failed"
fi

echo -e "\n🎯 Access URLs:"
echo "   Grafana:    http://localhost:3000 (admin/${GRAFANA_ADMIN_PASSWORD})"
echo "   Prometheus: http://localhost:9090"
echo "   Neural API: http://localhost:8080/health"
echo "   Data API:   http://localhost:8001/health"
```

## 🚨 Troubleshooting Guide

### Common Issues and Solutions

#### Issue 1: Services Not Starting

```bash
# Check container logs
docker-compose logs [service-name]

# Common fixes:
# 1. Port conflicts
netstat -tuln | grep [PORT]
# Solution: Change port mapping or stop conflicting service

# 2. Permission errors
sudo chown -R $USER:$USER /opt/neural-trader/
sudo chmod -R 755 /opt/neural-trader/

# 3. Environment variables missing
source .env && env | grep -E "(POSTGRES|GRAFANA|ALPACA)"
```

#### Issue 2: Database Connection Failures

```bash
# Verify TimescaleDB is running
docker-compose exec timescaledb pg_isready -U postgres

# Check connection from application
docker-compose exec neural-trader curl http://localhost:8080/health

# Manual database connection test
docker-compose exec timescaledb psql -U postgres -d neural_trader -c "SELECT version();"

# Reset database if corrupted
docker-compose down
docker volume rm production_timescaledb_data
docker-compose up -d timescaledb
```

#### Issue 3: Missing Metrics in Prometheus

```bash
# Check Prometheus targets
curl -s http://localhost:9090/api/v1/targets | jq '.data.activeTargets[] | {job: .labels.job, health: .health, error: .lastError}'

# Verify metrics endpoints
curl http://localhost:8080/metrics | grep neural_trader
curl http://localhost:8001/metrics | grep data_ingestion

# Restart metrics collection
docker-compose restart prometheus
```

#### Issue 4: Grafana Dashboards Not Loading

```bash
# Check Grafana logs
docker-compose logs grafana | grep -i error

# Verify dashboard files
ls -la grafana/dashboards/*.json

# Re-provision dashboards
docker-compose restart grafana

# Manual dashboard import via API
curl -u admin:${GRAFANA_ADMIN_PASSWORD} -X POST http://localhost:3000/api/dashboards/db \
  -H "Content-Type: application/json" \
  -d @grafana/dashboards/operational-overview.json
```

#### Issue 5: Performance Issues

```bash
# Check resource usage
docker stats

# Monitor application logs for bottlenecks
docker-compose logs neural-trader | grep -i -E "(slow|timeout|error)"

# Check database performance
docker-compose exec timescaledb psql -U postgres -d neural_trader -c "
  SELECT query, calls, total_time, mean_time 
  FROM pg_stat_statements 
  ORDER BY total_time DESC 
  LIMIT 10;"

# Redis memory usage
docker-compose exec redis redis-cli info memory
```

## 🔄 Rollback Procedures

### Emergency Rollback - Complete Stack

```bash
#!/bin/bash
# emergency_rollback.sh

echo "🚨 EMERGENCY ROLLBACK INITIATED - $(date)"

# Stop all services immediately
echo "⏹️ Stopping all services..."
docker-compose down

# Backup current configuration
echo "💾 Backing up current configuration..."
timestamp=$(date +%Y%m%d_%H%M%S)
tar -czf "config_backup_${timestamp}.tar.gz" configs/ grafana/ .env docker-compose.yml

# Restore from last known good configuration
echo "🔄 Restoring last known good configuration..."
if [[ -f "config_backup_last_good.tar.gz" ]]; then
  tar -xzf config_backup_last_good.tar.gz
  echo "✅ Configuration restored from backup"
else
  echo "❌ No backup found - manual intervention required"
  exit 1
fi

# Restart with previous configuration
echo "🚀 Restarting services with previous configuration..."
docker-compose up -d

# Verify rollback success
sleep 30
echo "🔍 Verifying rollback..."
if curl -f http://localhost:8080/health && curl -f http://localhost:3000/api/health; then
  echo "✅ Rollback successful - services are healthy"
else
  echo "❌ Rollback failed - manual intervention required"
  exit 1
fi
```

### Partial Rollback - Single Service

```bash
# Rollback specific service
SERVICE_NAME="neural-trader"  # or data-ingestion, grafana, etc.

echo "🔄 Rolling back $SERVICE_NAME..."

# Stop the problematic service
docker-compose stop $SERVICE_NAME

# Restore service-specific configuration
if [[ -f "configs/${SERVICE_NAME}_backup.yml" ]]; then
  cp "configs/${SERVICE_NAME}_backup.yml" "configs/${SERVICE_NAME}.yml"
fi

# Restart the service
docker-compose up -d $SERVICE_NAME

# Verify service health
timeout 60 bash -c "until docker-compose logs $SERVICE_NAME | grep -q 'started\|ready'; do sleep 5; done"
echo "✅ $SERVICE_NAME rollback completed"
```

### Database Rollback

```bash
# Database rollback procedure
echo "🗄️ Database rollback initiated..."

# Stop applications accessing database
docker-compose stop neural-trader data-ingestion

# Create current database backup
docker-compose exec timescaledb pg_dump -U postgres neural_trader > "db_backup_$(date +%Y%m%d_%H%M%S).sql"

# Restore from previous backup
if [[ -f "db_backup_last_good.sql" ]]; then
  docker-compose exec -T timescaledb dropdb -U postgres neural_trader
  docker-compose exec -T timescaledb createdb -U postgres neural_trader
  docker-compose exec -T timescaledb psql -U postgres neural_trader < db_backup_last_good.sql
  echo "✅ Database restored from backup"
else
  echo "❌ No database backup found"
  exit 1
fi

# Restart applications
docker-compose up -d neural-trader data-ingestion
```

## 📊 Performance Optimization

### Resource Tuning

```bash
# PostgreSQL optimization
docker-compose exec timescaledb psql -U postgres -d neural_trader -c "
  ALTER SYSTEM SET shared_buffers = '256MB';
  ALTER SYSTEM SET effective_cache_size = '1GB';
  ALTER SYSTEM SET work_mem = '4MB';
  SELECT pg_reload_conf();
"

# Redis optimization  
docker-compose exec redis redis-cli CONFIG SET maxmemory-policy allkeys-lru
docker-compose exec redis redis-cli CONFIG SET maxmemory 512mb

# Application JVM tuning (if applicable)
# Add to docker-compose.yml environment:
# - JAVA_OPTS=-Xmx2g -Xms1g -XX:+UseG1GC
```

### Monitoring Optimization

```bash
# Adjust Prometheus scrape intervals for high-frequency monitoring
# Edit configs/prometheus/prometheus.yml:
# scrape_interval: 5s  # For critical services
# scrape_interval: 30s # For less critical exporters

# Grafana performance settings
docker-compose exec grafana grafana-cli admin reset-admin-password ${GRAFANA_ADMIN_PASSWORD}
```

## 🛡️ Security Checklist

### Access Control

```bash
# Change default passwords
echo "🔐 Security checklist:"
echo "✅ Grafana admin password changed: ${GRAFANA_ADMIN_PASSWORD}"
echo "✅ PostgreSQL password set: ${POSTGRES_PASSWORD}"

# Verify no default credentials
if [[ "$GRAFANA_ADMIN_PASSWORD" == "admin" ]] || [[ "$POSTGRES_PASSWORD" == "password" ]]; then
  echo "❌ SECURITY RISK: Default passwords detected!"
  exit 1
fi

# Check for exposed ports
netstat -tuln | grep :5432  # Should only bind to container network
netstat -tuln | grep :6379  # Should only bind to container network
```

### Network Security

```bash
# Verify internal network isolation
docker network inspect production_neural-trader-network

# Check firewall rules (production)
# sudo ufw status
# sudo ufw allow 3000  # Grafana
# sudo ufw allow 9090  # Prometheus
# sudo ufw deny 5432   # PostgreSQL (internal only)
# sudo ufw deny 6379   # Redis (internal only)
```

## 📚 Additional Resources

### Useful Commands

```bash
# View all logs
docker-compose logs -f

# Check resource usage
docker stats

# Backup entire deployment
docker-compose down
tar -czf neural_trader_backup_$(date +%Y%m%d).tar.gz . 

# Update images
docker-compose pull
docker-compose up -d

# Scale services (if configured)
docker-compose up -d --scale data-ingestion=2
```

### Dashboard URLs

- **Grafana Main**: http://localhost:3000
- **Operational Overview**: http://localhost:3000/d/operational-overview
- **Trading Operations**: http://localhost:3000/d/trading-operations  
- **Market Data Realtime**: http://localhost:3000/d/market-data-realtime
- **Infrastructure**: http://localhost:3000/d/infrastructure-monitoring
- **Performance**: http://localhost:3000/d/performance-monitoring
- **Prometheus**: http://localhost:9090
- **Neural Trader API**: http://localhost:8080
- **Data Ingestion API**: http://localhost:8001

### Key Metrics to Monitor

1. **System Health**: CPU, Memory, Disk, Network
2. **Application Health**: Response times, error rates, throughput
3. **Database Health**: Connection count, query performance, replication lag
4. **Trading Metrics**: P&L, positions, risk exposure
5. **Neural Predictions**: Model accuracy, prediction latency, training metrics

---

## 📋 Deployment Completion Checklist

- [ ] All containers running (`docker-compose ps`)
- [ ] Database accessible and initialized
- [ ] Application APIs responding (`/health` endpoints)
- [ ] Prometheus collecting metrics from all targets
- [ ] Grafana dashboards loading with data
- [ ] Alert rules configured and firing appropriately
- [ ] Security credentials changed from defaults
- [ ] Backup procedures tested
- [ ] Rollback procedures documented and tested
- [ ] Monitoring alerts configured
- [ ] Performance baselines established

**Deployment Status**: ✅ Ready for Production Use

---

*This runbook was generated as part of the Neural Trader Production Validation initiative. For questions or issues, refer to the troubleshooting section above or consult the development team.*