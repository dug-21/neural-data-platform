# Neural Trader Platform Deployment Guide

## Overview

This guide covers production deployment of the Neural Trader Autonomous Platform, including infrastructure setup, configuration management, monitoring, and operational procedures.

## Prerequisites

### Hardware Requirements

#### Minimum Requirements
- **CPU**: 4 cores (8 recommended)
- **Memory**: 8GB RAM (16GB recommended)
- **Storage**: 100GB SSD (500GB recommended)
- **Network**: 1Gbps connection

#### Production Requirements
- **CPU**: 16+ cores with AVX2 support
- **Memory**: 32GB+ RAM (64GB for ML workloads)
- **Storage**: 1TB+ NVMe SSD with high IOPS
- **Network**: 10Gbps+ low-latency connection
- **GPU**: Optional NVIDIA GPU for ML acceleration

### Software Requirements
- **Operating System**: Ubuntu 20.04+ LTS, RHEL 8+, or Docker
- **Container Runtime**: Docker 20.10+ or Podman
- **Orchestration**: Kubernetes 1.21+ (optional)
- **Database**: PostgreSQL 13+ with TimescaleDB extension
- **Cache**: Redis 6+

## Deployment Options

### 1. Single Node Deployment (Small Scale)

Best for development, testing, and small-scale production deployments.

#### Step 1: Prepare the Server

```bash
# Update system packages
sudo apt update && sudo apt upgrade -y

# Install Docker
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
sudo usermod -aG docker $USER

# Install Docker Compose
sudo curl -L "https://github.com/docker/compose/releases/latest/download/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose
sudo chmod +x /usr/local/bin/docker-compose

# Logout and login to apply group changes
```

#### Step 2: Deploy Infrastructure

```bash
# Clone the repository
git clone <repository-url>
cd neural-trader

# Create production environment file
cp .env.example .env.production
# Edit .env.production with your configuration

# Start infrastructure services
sudo docker-compose -f docker-compose.prod.yml up -d postgres redis

# Wait for services to be ready
sleep 30

# Initialize database
sudo docker-compose -f docker-compose.prod.yml exec postgres psql -U neural_trader -d neural_trader_db -f /docker-entrypoint-initdb.d/init-db.sql
```

#### Step 3: Build and Deploy Application

```bash
# Build the application
cargo build --release

# Create application user
sudo useradd -r -s /bin/false neural-trader

# Create directories
sudo mkdir -p /opt/neural-trader/{bin,config,logs,data}
sudo chown -R neural-trader:neural-trader /opt/neural-trader

# Copy binaries and configuration
sudo cp target/release/autonomous-platform /opt/neural-trader/bin/
sudo cp config/platform.toml /opt/neural-trader/config/
sudo cp -r examples /opt/neural-trader/

# Create systemd service
sudo tee /etc/systemd/system/neural-trader.service > /dev/null <<EOF
[Unit]
Description=Neural Trader Autonomous Platform
After=network.target docker.service
Requires=docker.service

[Service]
Type=simple
User=neural-trader
Group=neural-trader
WorkingDirectory=/opt/neural-trader
ExecStart=/opt/neural-trader/bin/autonomous-platform
Restart=always
RestartSec=10
Environment=RUST_LOG=info
Environment=CONFIG_PATH=/opt/neural-trader/config/platform.toml

[Install]
WantedBy=multi-user.target
EOF

# Enable and start service
sudo systemctl daemon-reload
sudo systemctl enable neural-trader
sudo systemctl start neural-trader
```

### 2. Kubernetes Deployment (Production Scale)

Best for production deployments requiring high availability and scalability.

#### Step 1: Prepare Kubernetes Cluster

```bash
# Install kubectl
curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
sudo install -o root -g root -m 0755 kubectl /usr/local/bin/kubectl

# Install Helm
curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash
```

#### Step 2: Create Kubernetes Manifests

```yaml
# namespace.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: neural-trader
---
# configmap.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: neural-trader-config
  namespace: neural-trader
data:
  platform.toml: |
    [platform]
    name = "neural-trader-autonomous"
    version = "0.1.0"
    
    [database]
    url = "postgres://neural_trader:neural_trader_pass@postgres-service:5432/neural_trader_db"
    max_connections = 20
    min_connections = 5
    
    [redis]
    url = "redis://redis-service:6379"
    max_connections = 10
    default_ttl_seconds = 3600
    
    [neural]
    memory_gb = 4.0
    models = ["NHITS", "DeepAR", "TCN", "MLP"]
    prediction_cache_ttl = 300
    
    [monitoring]
    metrics_interval_secs = 60
    quality_threshold = 0.95
---
# postgres-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: postgres
  namespace: neural-trader
spec:
  replicas: 1
  selector:
    matchLabels:
      app: postgres
  template:
    metadata:
      labels:
        app: postgres
    spec:
      containers:
      - name: postgres
        image: timescale/timescaledb:latest-pg13
        ports:
        - containerPort: 5432
        env:
        - name: POSTGRES_DB
          value: neural_trader_db
        - name: POSTGRES_USER
          value: neural_trader
        - name: POSTGRES_PASSWORD
          value: neural_trader_pass
        volumeMounts:
        - name: postgres-storage
          mountPath: /var/lib/postgresql/data
      volumes:
      - name: postgres-storage
        persistentVolumeClaim:
          claimName: postgres-pvc
---
# postgres-service.yaml
apiVersion: v1
kind: Service
metadata:
  name: postgres-service
  namespace: neural-trader
spec:
  selector:
    app: postgres
  ports:
  - port: 5432
    targetPort: 5432
---
# redis-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: redis
  namespace: neural-trader
spec:
  replicas: 1
  selector:
    matchLabels:
      app: redis
  template:
    metadata:
      labels:
        app: redis
    spec:
      containers:
      - name: redis
        image: redis:6-alpine
        ports:
        - containerPort: 6379
        command: ["redis-server", "--appendonly", "yes"]
        volumeMounts:
        - name: redis-storage
          mountPath: /data
      volumes:
      - name: redis-storage
        persistentVolumeClaim:
          claimName: redis-pvc
---
# redis-service.yaml
apiVersion: v1
kind: Service
metadata:
  name: redis-service
  namespace: neural-trader
spec:
  selector:
    app: redis
  ports:
  - port: 6379
    targetPort:6379
---
# neural-trader-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: neural-trader
  namespace: neural-trader
spec:
  replicas: 3
  selector:
    matchLabels:
      app: neural-trader
  template:
    metadata:
      labels:
        app: neural-trader
    spec:
      containers:
      - name: neural-trader
        image: neural-trader:latest
        ports:
        - containerPort: 8080
        env:
        - name: RUST_LOG
          value: info
        - name: CONFIG_PATH
          value: /etc/neural-trader/platform.toml
        volumeMounts:
        - name: config-volume
          mountPath: /etc/neural-trader
        resources:
          requests:
            memory: "2Gi"
            cpu: "500m"
          limits:
            memory: "4Gi"
            cpu: "2000m"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
      volumes:
      - name: config-volume
        configMap:
          name: neural-trader-config
```

#### Step 3: Deploy to Kubernetes

```bash
# Apply Kubernetes manifests
kubectl apply -f k8s/

# Check deployment status
kubectl get pods -n neural-trader
kubectl get services -n neural-trader

# Check logs
kubectl logs -f deployment/neural-trader -n neural-trader
```

## Environment Configuration

### Environment Variables

```bash
# Database Configuration
export DATABASE_URL="postgres://user:pass@host:5432/db"
export DATABASE_MAX_CONNECTIONS=20
export DATABASE_MIN_CONNECTIONS=5

# Redis Configuration
export REDIS_URL="redis://host:6379"
export REDIS_MAX_CONNECTIONS=10
export REDIS_DEFAULT_TTL_SECONDS=3600

# Neural Network Configuration
export NEURAL_MEMORY_GB=4.0
export NEURAL_MODELS="NHITS,DeepAR,TCN,MLP"
export NEURAL_PREDICTION_CACHE_TTL=300

# Monitoring Configuration
export MONITORING_METRICS_INTERVAL_SECS=60
export MONITORING_QUALITY_THRESHOLD=0.95

# Logging Configuration
export RUST_LOG=info
export LOG_FORMAT=json
```

### Production Configuration Template

```toml
[platform]
name = "neural-trader-production"
version = "0.1.0"

[database]
url = "postgres://neural_trader:${DATABASE_PASSWORD}@postgres-cluster:5432/neural_trader_db"
max_connections = 50
min_connections = 10

[redis]
url = "redis://redis-cluster:6379"
max_connections = 20
default_ttl_seconds = 3600

[neural]
memory_gb = 8.0
models = ["NHITS", "DeepAR", "TCN", "MLP", "LSTM", "Transformer"]
prediction_cache_ttl = 300

[monitoring]
metrics_interval_secs = 30
quality_threshold = 0.98
```

## SSL/TLS Configuration

### Certificate Management

```bash
# Generate self-signed certificates for development
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
  -keyout /opt/neural-trader/ssl/server.key \
  -out /opt/neural-trader/ssl/server.crt \
  -subj "/CN=neural-trader.local"

# For production, use Let's Encrypt
sudo snap install --classic certbot
sudo certbot certonly --standalone -d your-domain.com
```

### Nginx Reverse Proxy Configuration

```nginx
# /etc/nginx/sites-available/neural-trader
server {
    listen 443 ssl http2;
    server_name your-domain.com;

    ssl_certificate /etc/letsencrypt/live/your-domain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/your-domain.com/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-RSA-AES256-GCM-SHA512:DHE-RSA-AES256-GCM-SHA512;
    ssl_prefer_server_ciphers off;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # WebSocket support
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }

    location /health {
        access_log off;
        proxy_pass http://127.0.0.1:8080/health;
    }
}

# Redirect HTTP to HTTPS
server {
    listen 80;
    server_name your-domain.com;
    return 301 https://$server_name$request_uri;
}
```

## Monitoring and Logging

### Prometheus Configuration

```yaml
# prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'neural-trader'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: '/metrics'
    scrape_interval: 30s
```

### Grafana Dashboard Setup

```bash
# Install Grafana
sudo apt-get install -y apt-transport-https software-properties-common wget
wget -q -O - https://packages.grafana.com/gpg.key | sudo apt-key add -
echo "deb https://packages.grafana.com/oss/deb stable main" | sudo tee -a /etc/apt/sources.list.d/grafana.list
sudo apt-get update
sudo apt-get install grafana

# Start Grafana
sudo systemctl enable grafana-server
sudo systemctl start grafana-server

# Access Grafana at http://localhost:3000
# Default credentials: admin/admin
```

### Log Management

```bash
# Install ELK Stack for log aggregation
docker-compose -f docker-compose.elk.yml up -d

# Configure Filebeat for log shipping
# /etc/filebeat/filebeat.yml
filebeat.inputs:
- type: log
  enabled: true
  paths:
    - /var/log/neural-trader/*.log
    - /opt/neural-trader/logs/*.log
  json.keys_under_root: true
  json.message_key: message

output.elasticsearch:
  hosts: ["localhost:9200"]

# Start Filebeat
sudo systemctl enable filebeat
sudo systemctl start filebeat
```

## Security Hardening

### Firewall Configuration

```bash
# Configure UFW firewall
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw allow ssh
sudo ufw allow 443/tcp  # HTTPS
sudo ufw allow 80/tcp   # HTTP (for redirect)
sudo ufw --force enable
```

### Database Security

```sql
-- Create application user with limited privileges
CREATE USER neural_trader_app WITH PASSWORD 'secure_password';
GRANT CONNECT ON DATABASE neural_trader_db TO neural_trader_app;
GRANT USAGE ON SCHEMA public TO neural_trader_app;
GRANT SELECT, INSERT, UPDATE ON ALL TABLES IN SCHEMA public TO neural_trader_app;
GRANT USAGE ON ALL SEQUENCES IN SCHEMA public TO neural_trader_app;

-- Enable SSL connections
-- In postgresql.conf:
-- ssl = on
-- ssl_cert_file = 'server.crt'
-- ssl_key_file = 'server.key'
```

### Application Security

```bash
# Run application as non-root user
sudo useradd -r -s /bin/false neural-trader

# Set proper file permissions
sudo chown -R neural-trader:neural-trader /opt/neural-trader
sudo chmod 750 /opt/neural-trader
sudo chmod 640 /opt/neural-trader/config/*
sudo chmod 750 /opt/neural-trader/bin/*

# Use secrets management
# Store sensitive data in environment variables or secret management systems
export DATABASE_PASSWORD=$(cat /run/secrets/db_password)
export REDIS_AUTH_TOKEN=$(cat /run/secrets/redis_token)
```

## Backup and Recovery

### Database Backup

```bash
# Daily backup script
#!/bin/bash
DATE=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="/opt/backups/neural-trader"
DB_NAME="neural_trader_db"

# Create backup directory
mkdir -p $BACKUP_DIR

# Perform backup
pg_dump -h localhost -U neural_trader -d $DB_NAME | gzip > $BACKUP_DIR/neural_trader_$DATE.sql.gz

# Keep only last 7 days of backups
find $BACKUP_DIR -name "neural_trader_*.sql.gz" -mtime +7 -delete

# Add to crontab: 0 2 * * * /opt/scripts/backup_db.sh
```

### Application State Backup

```bash
# Backup configuration and logs
tar -czf /opt/backups/neural-trader-state-$(date +%Y%m%d).tar.gz \
  /opt/neural-trader/config \
  /opt/neural-trader/logs \
  /opt/neural-trader/data
```

### Recovery Procedures

```bash
# Database recovery
zcat /opt/backups/neural-trader/neural_trader_YYYYMMDD_HHMMSS.sql.gz | \
  psql -h localhost -U neural_trader -d neural_trader_db

# Application state recovery
tar -xzf /opt/backups/neural-trader-state-YYYYMMDD.tar.gz -C /
sudo systemctl restart neural-trader
```

## Performance Tuning

### Database Optimization

```sql
-- PostgreSQL configuration tuning
-- In postgresql.conf:
shared_buffers = 2GB
effective_cache_size = 6GB
maintenance_work_mem = 512MB
work_mem = 64MB
max_connections = 100
random_page_cost = 1.1
effective_io_concurrency = 200

-- TimescaleDB specific optimizations
SELECT timescaledb_tune_chunk_sizing();
SELECT set_chunk_time_interval('market_data', INTERVAL '1 hour');
```

### Redis Optimization

```bash
# Redis configuration
# In redis.conf:
maxmemory 4gb
maxmemory-policy allkeys-lru
save 900 1
save 300 10
save 60 10000
```

### Application Optimization

```bash
# Rust optimization flags
export RUSTFLAGS="-C target-cpu=native -C opt-level=3"

# System optimization
echo 'vm.swappiness = 10' >> /etc/sysctl.conf
echo 'net.core.rmem_max = 16777216' >> /etc/sysctl.conf
echo 'net.core.wmem_max = 16777216' >> /etc/sysctl.conf
sysctl -p
```

## Troubleshooting

### Common Issues

1. **High Memory Usage**
   ```bash
   # Check memory usage
   free -h
   ps aux --sort=-%mem | head -10
   
   # Reduce neural model memory allocation
   export NEURAL_MEMORY_GB=2.0
   ```

2. **Database Connection Issues**
   ```bash
   # Check database connectivity
   psql -h localhost -U neural_trader -d neural_trader_db -c "SELECT version();"
   
   # Check connection pool status
   curl http://localhost:8080/metrics | grep db_connections
   ```

3. **High Latency**
   ```bash
   # Check network latency
   ping -c 5 database-host
   
   # Monitor application performance
   curl http://localhost:8080/metrics | grep request_duration
   ```

### Log Analysis

```bash
# View application logs
journalctl -u neural-trader -f

# Search for errors
grep -i error /opt/neural-trader/logs/*.log

# Monitor performance metrics
tail -f /opt/neural-trader/logs/metrics.log
```

## Maintenance Procedures

### Regular Maintenance Tasks

1. **Weekly**:
   - Review system logs for errors
   - Check disk space usage
   - Verify backup integrity
   - Update security patches

2. **Monthly**:
   - Analyze performance metrics
   - Review and optimize database queries
   - Update dependencies
   - Test disaster recovery procedures

3. **Quarterly**:
   - Review and update security configurations
   - Performance benchmarking
   - Capacity planning review
   - Update documentation

### Update Procedures

```bash
# Update application
# 1. Download new version
wget https://releases.neural-trader.com/v0.2.0/neural-trader-linux-x64.tar.gz

# 2. Stop service
sudo systemctl stop neural-trader

# 3. Backup current version
sudo cp /opt/neural-trader/bin/autonomous-platform /opt/neural-trader/bin/autonomous-platform.backup

# 4. Install new version
tar -xzf neural-trader-linux-x64.tar.gz
sudo cp autonomous-platform /opt/neural-trader/bin/

# 5. Update configuration if needed
# Review config changes in release notes

# 6. Start service
sudo systemctl start neural-trader

# 7. Verify deployment
curl http://localhost:8080/health
```

This deployment guide provides comprehensive instructions for production deployment of the Neural Trader Autonomous Platform. Always test deployments in a staging environment before applying to production.