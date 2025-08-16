# Neural Trader Containerization Implementation Guide

## Quick Start

### 1. Prerequisites

```bash
# Install Docker and Docker Compose
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh

# Install Docker Compose
sudo curl -L "https://github.com/docker/compose/releases/latest/download/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose
sudo chmod +x /usr/local/bin/docker-compose

# For Kubernetes deployment
kubectl version --client
helm version
```

### 2. Environment Setup

```bash
# Clone the repository
git clone <repository-url>
cd neural-trader

# Create environment file
cp .env.example .env

# Edit environment variables
nano .env
```

Required environment variables:
```bash
# Database
POSTGRES_USER=neural_trader
POSTGRES_DB=neural_trader_db
POSTGRES_PASSWORD=secure_password

# Redis
REDIS_URL=redis://redis:6379

# API Keys
ALPACA_API_KEY=your_alpaca_key
ALPACA_API_SECRET=your_alpaca_secret
POLYGON_API_KEY=your_polygon_key
FINNHUB_API_KEY=your_finnhub_key

# Features
ENABLE_SECTOR_MODELS=true
ENABLE_REALTIME_ADAPTATION=true
ENABLE_AUTONOMOUS_TRAINING=false
NEURAL_USE_REAL_MODELS=true

# Monitoring
GRAFANA_ADMIN_PASSWORD=admin_password
LOG_LEVEL=info
```

### 3. Local Development

```bash
# Build and start all services
docker-compose -f docker/docker-compose.modular.yml up -d

# Check service status
docker-compose -f docker/docker-compose.modular.yml ps

# View logs
docker-compose -f docker/docker-compose.modular.yml logs -f neural-trader

# Access services
curl http://localhost:8080/health          # Neural Trader
curl http://localhost:8001/health          # Data Ingestion
curl http://localhost:8002/health          # Model Manager
open http://localhost:3000                 # Grafana
open http://localhost:9090                 # Prometheus
```

## Architecture Overview

### Service Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Load Balancer                        │
│                     (Nginx)                            │
└─────────────────────┬───────────────────────────────────┘
                      │
┌─────────────────────┼───────────────────────────────────┐
│                     │        Frontend Network           │
│  ┌─────────────┐   │   ┌─────────────┐ ┌─────────────┐ │
│  │ Neural      │   │   │ Data        │ │ Model       │ │
│  │ Trader      │───┼───│ Ingestion   │ │ Manager     │ │
│  │ :8080       │   │   │ :8001       │ │ :8002       │ │
│  └─────────────┘   │   └─────────────┘ └─────────────┘ │
└─────────────────────┼───────────────────────────────────┘
                      │
┌─────────────────────┼───────────────────────────────────┐
│                     │        Backend Network            │
│  ┌─────────────┐   │   ┌─────────────┐                 │
│  │ TimescaleDB │   │   │ Redis       │                 │
│  │ :5432       │───┼───│ :6379       │                 │
│  └─────────────┘   │   └─────────────┘                 │
└─────────────────────┼───────────────────────────────────┘
                      │
┌─────────────────────┼───────────────────────────────────┐
│                     │      Monitoring Network           │
│  ┌─────────────┐   │   ┌─────────────┐                 │
│  │ Prometheus  │   │   │ Grafana     │                 │
│  │ :9090       │───┼───│ :3000       │                 │
│  └─────────────┘   │   └─────────────┘                 │
└─────────────────────────────────────────────────────────┘
```

### Container Boundaries

| Service | Purpose | Scaling | Dependencies |
|---------|---------|---------|--------------|
| `neural-trader` | Core trading engine | Horizontal | TimescaleDB, Redis |
| `data-ingestion` | Market data pipeline | Horizontal | TimescaleDB, Redis |
| `model-manager` | ML model lifecycle | Vertical | Neural Trader, Storage |
| `timescaledb` | Time-series database | Vertical | None |
| `redis` | Cache & messaging | Vertical | None |
| `nginx` | Load balancer | Horizontal | All services |
| `prometheus` | Metrics collection | Vertical | All services |
| `grafana` | Visualization | Vertical | Prometheus, TimescaleDB |

## Deployment Options

### 1. Development Environment

```bash
# Start with development overrides
docker-compose -f docker/docker-compose.modular.yml \
               -f docker/docker-compose.dev.yml up -d

# Enable hot reloading
docker-compose exec neural-trader cargo watch -x run

# Run tests
docker-compose exec neural-trader cargo test
docker-compose exec data-ingestion python -m pytest
```

### 2. Production Environment

```bash
# Build production images
./docker/base/scripts/build-production.sh

# Deploy with production settings
docker-compose -f docker/docker-compose.modular.yml \
               -f docker/docker-compose.prod.yml up -d

# Scale services
docker-compose -f docker/docker-compose.modular.yml scale data-ingestion=3

# Monitor deployment
docker-compose -f docker/docker-compose.modular.yml logs -f
```

### 3. Kubernetes Deployment

```bash
# Create namespace
kubectl apply -f k8s/neural-trader-deployment.yaml

# Verify deployment
kubectl get pods -n neural-trader
kubectl get services -n neural-trader

# Check logs
kubectl logs -f deployment/neural-trader -n neural-trader

# Scale services
kubectl scale deployment neural-trader --replicas=5 -n neural-trader

# Port forward for local access
kubectl port-forward service/neural-trader 8080:8080 -n neural-trader
```

## Volume Strategy

### Shared Data Volumes

1. **Model Storage** (`neural_models`)
   - **Type**: tmpfs (in-memory)
   - **Size**: 8GB
   - **Access**: ReadWrite for neural-trader and model-manager
   - **Purpose**: High-performance model serving

2. **Market Data Cache** (`market_data_cache`)
   - **Type**: Bind mount (fast SSD)
   - **Size**: Variable
   - **Access**: ReadWrite for data-ingestion, ReadOnly for neural-trader
   - **Purpose**: Zero-copy data sharing

3. **Configuration** (`shared_config`)
   - **Type**: Bind mount (read-only)
   - **Access**: ReadOnly for all services
   - **Purpose**: Centralized configuration

### Volume Performance Optimization

```yaml
# High-performance storage for models
volumes:
  neural_models:
    driver: local
    driver_opts:
      type: tmpfs
      device: tmpfs
      o: size=8g,uid=1000,gid=1000

# Fast SSD storage for market data
  market_data_cache:
    driver: local
    driver_opts:
      type: bind
      device: /mnt/fast-ssd/market-data
```

## Network Configuration

### Network Isolation

1. **Frontend Network** (172.20.0.0/16)
   - External access
   - Load balancer and API services
   
2. **Backend Network** (172.21.0.0/16)
   - Internal services only
   - Database and cache layers
   
3. **Monitoring Network** (172.22.0.0/16)
   - Metrics collection
   - Prometheus and Grafana

### Service Discovery

Services discover each other using Docker's built-in DNS:
- `timescaledb:5432`
- `redis:6379`
- `neural-trader:8080`
- `data-ingestion:8001`
- `model-manager:8002`

## Health Checks and Monitoring

### Health Check Endpoints

| Service | Health Endpoint | Ready Endpoint |
|---------|----------------|----------------|
| Neural Trader | `/health` | `/ready` |
| Data Ingestion | `/health` | `/ready` |
| Model Manager | `/health` | `/ready` |

### Monitoring Stack

1. **Prometheus**: Metrics collection
   - Service discovery via Docker labels
   - Custom metrics from each service
   - Alert rules for critical events

2. **Grafana**: Visualization
   - Pre-configured dashboards
   - Real-time monitoring
   - Alert notifications

3. **Exporters**: System metrics
   - PostgreSQL Exporter
   - Redis Exporter
   - Node Exporter

## Security Configuration

### Container Security

```yaml
# Security context for all containers
security_opt:
  - no-new-privileges:true
cap_drop:
  - ALL
cap_add:
  - NET_BIND_SERVICE  # Only for services needing port binding
read_only: true  # Where possible
```

### Secrets Management

```yaml
# Docker Compose secrets
secrets:
  jwt_secret:
    external: true
  api_keys:
    file: ./secrets/api_keys.json
  db_password:
    external: true
```

### Network Security

```yaml
# Network policies (Kubernetes)
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: neural-trader-policy
spec:
  podSelector:
    matchLabels:
      app: neural-trader
  policyTypes:
  - Ingress
  - Egress
```

## Scaling Strategies

### Horizontal Scaling

```bash
# Scale data ingestion
docker-compose -f docker/docker-compose.modular.yml scale data-ingestion=5

# Scale neural trader
kubectl scale deployment neural-trader --replicas=10 -n neural-trader
```

### Vertical Scaling

```yaml
# Increase resources
deploy:
  resources:
    limits:
      memory: 8G
      cpus: '4.0'
    reservations:
      memory: 4G
      cpus: '2.0'
```

### Auto-scaling (Kubernetes)

```yaml
# HPA configuration
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: neural-trader-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: neural-trader
  minReplicas: 2
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
```

## Troubleshooting

### Common Issues

1. **Container Won't Start**
   ```bash
   # Check logs
   docker-compose logs neural-trader
   
   # Check health
   docker-compose exec neural-trader curl localhost:9092/health
   ```

2. **Database Connection Issues**
   ```bash
   # Test connectivity
   docker-compose exec neural-trader nc -z timescaledb 5432
   
   # Check database logs
   docker-compose logs timescaledb
   ```

3. **Redis Connection Issues**
   ```bash
   # Test Redis
   docker-compose exec redis redis-cli ping
   
   # Check Redis config
   docker-compose exec redis redis-cli info
   ```

4. **Memory Issues**
   ```bash
   # Check memory usage
   docker stats
   
   # Adjust memory limits
   docker-compose -f docker-compose.yml -f docker-compose.prod.yml up -d
   ```

### Performance Optimization

1. **Database Tuning**
   ```sql
   -- Check TimescaleDB performance
   SELECT * FROM timescaledb_information.hypertables;
   SELECT * FROM timescaledb_information.chunks;
   ```

2. **Redis Optimization**
   ```bash
   # Monitor Redis performance
   docker-compose exec redis redis-cli --latency
   docker-compose exec redis redis-cli info memory
   ```

3. **Model Loading Optimization**
   ```bash
   # Check model loading times
   docker-compose logs model-manager | grep "model_load"
   ```

## Maintenance

### Backup and Recovery

```bash
# Database backup
docker-compose exec timescaledb pg_dump -U neural_trader neural_trader_db > backup.sql

# Redis backup
docker-compose exec redis redis-cli BGSAVE
docker cp neural-trader-redis:/data/dump.rdb ./redis-backup.rdb

# Model backup
docker run --rm -v neural_models:/data -v $(pwd):/backup alpine tar czf /backup/models-backup.tar.gz -C /data .
```

### Updates and Upgrades

```bash
# Update images
docker-compose pull

# Rolling update
docker-compose up -d --no-deps neural-trader

# Kubernetes rolling update
kubectl rollout restart deployment/neural-trader -n neural-trader
kubectl rollout status deployment/neural-trader -n neural-trader
```

### Log Management

```bash
# View logs
docker-compose logs -f --tail=100 neural-trader

# Log rotation (production)
docker-compose -f docker-compose.yml -f docker-compose.prod.yml up -d
```

## Performance Benchmarks

### Expected Performance

| Service | CPU Usage | Memory Usage | Response Time |
|---------|-----------|--------------|---------------|
| Neural Trader | 1-2 cores | 2-4GB | <100ms |
| Data Ingestion | 0.5-1 core | 1-2GB | <50ms |
| Model Manager | 2-4 cores | 4-8GB | <500ms |
| TimescaleDB | 1-2 cores | 2-4GB | <10ms |
| Redis | 0.5 cores | 1-2GB | <1ms |

### Load Testing

```bash
# API load test
hey -n 1000 -c 10 http://localhost:8080/api/v1/predict

# Database load test
pgbench -h localhost -p 5432 -U neural_trader -d neural_trader_db -c 10 -j 2 -t 1000

# Redis load test
redis-benchmark -h localhost -p 6379 -n 10000 -c 50
```

## Production Checklist

- [ ] Environment variables configured
- [ ] Secrets properly managed
- [ ] Resource limits set
- [ ] Health checks configured
- [ ] Monitoring enabled
- [ ] Backup strategy implemented
- [ ] Security policies applied
- [ ] Performance tested
- [ ] Documentation updated
- [ ] Incident response plan ready

This implementation provides a robust, scalable, and maintainable containerization strategy for the Neural Trader platform.