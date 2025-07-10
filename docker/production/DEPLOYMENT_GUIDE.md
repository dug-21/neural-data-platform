# Neural Trader Production Deployment Guide

## ✅ Images Successfully Built

The following production images are now available:

- `neural-trader:prod` (151MB) - Main application with real FANN neural networks
- `neural-trader/data-ingestion:prod` (~500MB) - Data ingestion service with Yahoo Finance, Finnhub, Alpha Vantage
- `neural-trader/timescaledb:prod` (1.27GB) - TimescaleDB with schema
- `neural-trader/prometheus:prod` (370MB) - Prometheus with alerts
- `neural-trader/grafana:prod` (847MB) - Grafana with dashboards

## 🚀 Quick Deployment

### 1. Inside DevContainer (Current Environment)

```bash
cd docker/production
./deploy.sh
```

This will:
- Check for required images
- Start all services with docker-compose
- Show health status
- Display access URLs

### 2. Export for External Deployment

```bash
# Save images
cd docker/production
docker save -o neural-trader-prod.tar \
  neural-trader:prod \
  neural-trader/data-ingestion:prod \
  neural-trader/timescaledb:prod \
  neural-trader/prometheus:prod \
  neural-trader/grafana:prod

# Transfer to production host
scp neural-trader-prod.tar user@host:/path/to/
scp docker-compose.prod.yml user@host:/path/to/
scp .env user@host:/path/to/
```

On production host:
```bash
# Load images
docker load -i neural-trader-prod.tar

# Start services
docker-compose -f docker-compose.prod.yml up -d
```

### 3. Push to Registry

```bash
# Tag for your registry
REGISTRY=your-registry.com
docker tag neural-trader:prod $REGISTRY/neural-trader:prod
docker tag neural-trader/data-ingestion:prod $REGISTRY/neural-trader/data-ingestion:prod
docker tag neural-trader/timescaledb:prod $REGISTRY/neural-trader/timescaledb:prod
docker tag neural-trader/prometheus:prod $REGISTRY/neural-trader/prometheus:prod
docker tag neural-trader/grafana:prod $REGISTRY/neural-trader/grafana:prod

# Push
docker push $REGISTRY/neural-trader:prod
docker push $REGISTRY/neural-trader/data-ingestion:prod
docker push $REGISTRY/neural-trader/timescaledb:prod
docker push $REGISTRY/neural-trader/prometheus:prod
docker push $REGISTRY/neural-trader/grafana:prod
```

## 📊 Data Persistence

All data is stored in Docker volumes:

- `timescaledb_data` - Market data, predictions, decisions
- `redis_data` - Cache data
- `prometheus_data` - Metrics history (30 days retention)
- `grafana_data` - Dashboards and user settings
- `neural_trader_data` - Application state
- `neural_trader_logs` - Application logs
- `data_ingestion_logs` - Data ingestion service logs

## 🔒 Security Checklist

Before deploying to production:

1. **Change all passwords in .env**
   - POSTGRES_PASSWORD
   - GRAFANA_PASSWORD
   - Redis password (if configured)

2. **Update firewall rules**
   - Only expose necessary ports
   - Use reverse proxy for public access

3. **Enable TLS/SSL**
   - For all external endpoints
   - Between services if possible

4. **Set resource limits**
   - Already configured in docker-compose.prod.yml
   - Adjust based on your hardware

## 🎯 Autonomous Trading

The neural trader will start automatically and:

1. **Initialize Neural Networks** - FANN models (NHITS, TCN, DeepAR, MLP)
2. **Connect to DAA Service** - For autonomous decision making
3. **Start Trading Loop** - Based on configured symbols and intervals
4. **Monitor Performance** - Via Prometheus metrics and Grafana

## 📈 Monitoring

- **Grafana Dashboard**: http://localhost:3000
  - Login: admin / [your password]
  - Pre-configured "Neural Trader Overview" dashboard
  
- **Prometheus**: http://localhost:9090
  - Query metrics directly
  - View configured alerts
  
- **Application Logs**:
  ```bash
  docker-compose -f docker-compose.prod.yml logs -f neural-trader
  ```

## 🛠️ Maintenance

### Update Application
```bash
# Rebuild image with new code
./build.sh

# Rolling update
docker-compose -f docker-compose.prod.yml up -d --no-deps neural-trader
```

### Backup Data
```bash
# Backup script included in deploy.sh
./deploy.sh backup
```

### Scale Services
```bash
# Scale neural-trader instances
docker-compose -f docker-compose.prod.yml up -d --scale neural-trader=3
```

## 🆘 Troubleshooting

### Check Service Health
```bash
docker-compose -f docker-compose.prod.yml ps
docker-compose -f docker-compose.prod.yml logs [service-name]
```

### Database Connection Issues
```bash
# Check TimescaleDB logs
docker-compose -f docker-compose.prod.yml logs timescaledb

# Test connection
docker-compose -f docker-compose.prod.yml exec timescaledb psql -U trader neural_trader
```

### Reset Everything
```bash
# Stop and remove everything (INCLUDING DATA)
docker-compose -f docker-compose.prod.yml down -v

# Start fresh
./deploy.sh
```