# Deployment Guide

## Overview

This guide covers deploying the Neural Trading Platform from development to production. The platform is designed for personal trading and optimized for single-machine deployment on macOS, but can be adapted for cloud deployment.

## Deployment Environments

### 1. Development Environment
- **Target**: Local macOS development
- **Purpose**: Development and testing
- **Scale**: Single developer, minimal resources

### 2. Staging Environment  
- **Target**: Local macOS or cloud VM
- **Purpose**: Pre-production testing
- **Scale**: Production-like data, reduced scale

### 3. Production Environment
- **Target**: Personal trading setup (macOS recommended)
- **Purpose**: Live trading operations
- **Scale**: Personal portfolio management

## Prerequisites

### System Requirements

#### Minimum Requirements
- **OS**: macOS 12+ (recommended), Linux (Ubuntu 20.04+)
- **CPU**: 4 cores, 2.4GHz+
- **RAM**: 8GB minimum, 16GB recommended
- **Storage**: 100GB SSD minimum
- **Network**: Stable internet connection (5+ Mbps)

#### Recommended Requirements
- **OS**: macOS 13+ with Apple Silicon
- **CPU**: 8 cores, M1/M2 or equivalent
- **RAM**: 32GB for optimal neural network performance
- **Storage**: 500GB+ NVMe SSD
- **Network**: Dedicated internet connection (25+ Mbps)
- **GPU**: Optional, for faster neural network training

### Software Dependencies

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Install Docker Desktop
# Download from: https://www.docker.com/products/docker-desktop

# Install Docker Compose (if not included with Docker Desktop)
brew install docker-compose

# Install additional tools
brew install git curl wget jq
```

## Configuration Management

### Environment-Specific Configurations

#### Development (.env.development)
```bash
# Development environment
ENVIRONMENT=development
DEBUG_MODE=true
RUST_LOG=debug

# Database (local containers)
DB_HOST=localhost
DB_PORT=5432
DB_NAME=trading_dev
DB_USER=trading_dev
DB_PASSWORD=dev_password

# Trading (simulation mode)
TRADING_MODE=simulation
INITIAL_CAPITAL=10000.00
DAA_ENABLED=true

# Data providers (sandbox/test keys)
IEX_API_KEY=pk_test_your_test_key_here
ALPACA_API_KEY=test_api_key
ALPACA_SECRET_KEY=test_secret_key

# Neural networks (reduced settings)
NEURAL_ENGINE_MEMORY_MB=512
TRAINING_ENABLED=true
```

#### Production (.env.production)
```bash
# Production environment
ENVIRONMENT=production
DEBUG_MODE=false
RUST_LOG=info

# Database (production settings)
DB_HOST=localhost
DB_PORT=5432
DB_NAME=trading_prod
DB_USER=trading_prod
DB_PASSWORD=super_secure_password_here

# Trading (live mode)
TRADING_MODE=live
INITIAL_CAPITAL=100000.00
DAA_ENABLED=true

# Data providers (production keys)
IEX_API_KEY=pk_your_production_key_here
ALPACA_API_KEY=your_production_api_key
ALPACA_SECRET_KEY=your_production_secret_key

# Neural networks (production settings)
NEURAL_ENGINE_MEMORY_MB=2048
TRAINING_ENABLED=true
GPU_ENABLED=false

# Security
JWT_SECRET=your_jwt_secret_key_here
ENCRYPTION_KEY=your_encryption_key_here

# Monitoring
METRICS_ENABLED=true
BACKUP_ENABLED=true
```

### Configuration Validation

```bash
#!/bin/bash
# scripts/validate-config.sh

echo "Validating configuration..."

# Check required environment variables
REQUIRED_VARS=(
    "DB_PASSWORD"
    "IEX_API_KEY" 
    "ALPACA_API_KEY"
    "ALPACA_SECRET_KEY"
    "INITIAL_CAPITAL"
)

for var in "${REQUIRED_VARS[@]}"; do
    if [ -z "${!var}" ]; then
        echo "ERROR: Required environment variable $var is not set"
        exit 1
    fi
done

# Validate database connection
if ! cargo run --bin validate-db-connection; then
    echo "ERROR: Database connection failed"
    exit 1
fi

# Validate API keys
if ! cargo run --bin validate-api-keys; then
    echo "ERROR: API key validation failed"
    exit 1
fi

echo "Configuration validation passed!"
```

## Build and Packaging

### Production Build

```bash
#!/bin/bash
# scripts/build-production.sh

set -e

echo "Building production release..."

# Clean previous builds
cargo clean

# Build with optimizations
cargo build --release --features daa,live-trading

# Strip debug symbols
strip target/release/trading-platform
strip target/release/market-data-ingestion
strip target/release/mcp-server

# Create deployment package
mkdir -p dist/neural-trading-platform
cp target/release/trading-platform dist/neural-trading-platform/
cp target/release/market-data-ingestion dist/neural-trading-platform/
cp target/release/mcp-server dist/neural-trading-platform/
cp -r config dist/neural-trading-platform/
cp -r scripts dist/neural-trading-platform/
cp docker-compose.yml dist/neural-trading-platform/
cp README.md dist/neural-trading-platform/

# Create archive
cd dist
tar -czf neural-trading-platform-$(date +%Y%m%d).tar.gz neural-trading-platform/
cd ..

echo "Production build complete: dist/neural-trading-platform-$(date +%Y%m%d).tar.gz"
```

### Docker Production Images

```dockerfile
# Dockerfile.production
FROM rust:1.75-slim as builder

WORKDIR /app

# Install dependencies
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev libpq-dev && \
    rm -rf /var/lib/apt/lists/*

# Copy source
COPY . .

# Build optimized release
RUN cargo build --release --features daa,live-trading

# Production runtime image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y \
        ca-certificates \
        libssl3 \
        libpq5 \
        curl && \
    rm -rf /var/lib/apt/lists/*

# Create trading user
RUN useradd -m -u 1001 -s /bin/bash trading

# Create directories
RUN mkdir -p /app/{config,logs,data,backups} && \
    chown -R trading:trading /app

WORKDIR /app

# Copy binaries
COPY --from=builder /app/target/release/trading-platform /usr/local/bin/
COPY --from=builder /app/target/release/market-data-ingestion /usr/local/bin/
COPY --from=builder /app/target/release/mcp-server /usr/local/bin/

# Copy configuration
COPY config/ ./config/
COPY scripts/ ./scripts/

# Set permissions
RUN chmod +x /usr/local/bin/* && \
    chmod +x ./scripts/*.sh

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:8081/health || exit 1

# Switch to trading user
USER trading

# Expose ports
EXPOSE 8080 8081 9091

# Environment
ENV RUST_LOG=info
ENV ENVIRONMENT=production

# Default command
CMD ["trading-platform"]
```

## Database Deployment

### Production Database Setup

```bash
#!/bin/bash
# scripts/setup-production-db.sh

set -e

echo "Setting up production database..."

# Start TimescaleDB container with production settings
docker run -d \
    --name neural-trading-timescaledb-prod \
    --restart unless-stopped \
    -p 5432:5432 \
    -e POSTGRES_DB=${DB_NAME} \
    -e POSTGRES_USER=${DB_USER} \
    -e POSTGRES_PASSWORD=${DB_PASSWORD} \
    -e TIMESCALEDB_TELEMETRY=off \
    -v neural_trading_data_prod:/var/lib/postgresql/data \
    -v $(pwd)/docker/data-platform/timescaledb/init:/docker-entrypoint-initdb.d \
    timescale/timescaledb:latest-pg15 \
    postgres \
    -c shared_preload_libraries=timescaledb \
    -c max_connections=200 \
    -c shared_buffers=256MB \
    -c effective_cache_size=1GB \
    -c maintenance_work_mem=64MB \
    -c checkpoint_completion_target=0.9 \
    -c wal_buffers=16MB \
    -c default_statistics_target=100

# Wait for database to be ready
echo "Waiting for database to be ready..."
until docker exec neural-trading-timescaledb-prod pg_isready -U ${DB_USER}; do
    sleep 2
done

# Run migrations
echo "Running database migrations..."
cargo run --release --bin migrate

# Create backup user
echo "Creating backup user..."
docker exec neural-trading-timescaledb-prod psql -U ${DB_USER} -d ${DB_NAME} -c \
    "CREATE USER backup_user WITH PASSWORD '${BACKUP_PASSWORD}';" || true
docker exec neural-trading-timescaledb-prod psql -U ${DB_USER} -d ${DB_NAME} -c \
    "GRANT SELECT ON ALL TABLES IN SCHEMA public TO backup_user;" || true

echo "Database setup complete!"
```

### Database Backup Strategy

```bash
#!/bin/bash
# scripts/backup-database.sh

set -e

BACKUP_DIR="/app/backups"
DATE=$(date +%Y%m%d_%H%M%S)
BACKUP_FILE="trading_backup_${DATE}.sql"

echo "Creating database backup: ${BACKUP_FILE}"

# Create backup directory
mkdir -p ${BACKUP_DIR}

# Create full backup
docker exec neural-trading-timescaledb-prod pg_dump \
    -U ${DB_USER} \
    -h localhost \
    -d ${DB_NAME} \
    --verbose \
    --no-password \
    > ${BACKUP_DIR}/${BACKUP_FILE}

# Compress backup
gzip ${BACKUP_DIR}/${BACKUP_FILE}

# Keep only last 30 days of backups
find ${BACKUP_DIR} -name "trading_backup_*.sql.gz" -mtime +30 -delete

# Upload to cloud storage (optional)
if [ "${BACKUP_TO_CLOUD}" = "true" ]; then
    # Add your cloud storage upload command here
    echo "Uploading backup to cloud storage..."
fi

echo "Backup completed: ${BACKUP_DIR}/${BACKUP_FILE}.gz"
```

## Application Deployment

### Production Deployment Script

```bash
#!/bin/bash
# scripts/deploy-production.sh

set -e

DEPLOYMENT_DIR="/opt/neural-trading-platform"
SERVICE_USER="trading"
BACKUP_DIR="/opt/neural-trading-platform/backups"

echo "Deploying Neural Trading Platform to production..."

# Create service user if not exists
if ! id "$SERVICE_USER" &>/dev/null; then
    echo "Creating service user: $SERVICE_USER"
    sudo useradd -m -s /bin/bash $SERVICE_USER
fi

# Create deployment directory
sudo mkdir -p $DEPLOYMENT_DIR
sudo chown $SERVICE_USER:$SERVICE_USER $DEPLOYMENT_DIR

# Stop existing services
echo "Stopping existing services..."
sudo systemctl stop neural-trading-platform || true
sudo systemctl stop neural-trading-mcp-server || true

# Backup current deployment
if [ -d "$DEPLOYMENT_DIR/current" ]; then
    echo "Backing up current deployment..."
    sudo mv $DEPLOYMENT_DIR/current $DEPLOYMENT_DIR/backup-$(date +%Y%m%d_%H%M%S)
fi

# Extract new deployment
echo "Extracting new deployment..."
sudo mkdir -p $DEPLOYMENT_DIR/current
sudo tar -xzf dist/neural-trading-platform-*.tar.gz -C $DEPLOYMENT_DIR/current --strip-components=1
sudo chown -R $SERVICE_USER:$SERVICE_USER $DEPLOYMENT_DIR/current

# Copy environment configuration
echo "Copying production configuration..."
sudo cp .env.production $DEPLOYMENT_DIR/current/.env
sudo chown $SERVICE_USER:$SERVICE_USER $DEPLOYMENT_DIR/current/.env
sudo chmod 600 $DEPLOYMENT_DIR/current/.env

# Install systemd services
echo "Installing systemd services..."
sudo cp $DEPLOYMENT_DIR/current/scripts/systemd/*.service /etc/systemd/system/
sudo systemctl daemon-reload

# Start services
echo "Starting services..."
sudo systemctl enable neural-trading-platform
sudo systemctl enable neural-trading-mcp-server
sudo systemctl start neural-trading-platform
sudo systemctl start neural-trading-mcp-server

# Verify deployment
echo "Verifying deployment..."
sleep 10

if sudo systemctl is-active --quiet neural-trading-platform; then
    echo "✅ Trading platform is running"
else
    echo "❌ Trading platform failed to start"
    sudo journalctl -u neural-trading-platform --no-pager -n 20
    exit 1
fi

if sudo systemctl is-active --quiet neural-trading-mcp-server; then
    echo "✅ MCP server is running"
else
    echo "❌ MCP server failed to start"
    sudo journalctl -u neural-trading-mcp-server --no-pager -n 20
    exit 1
fi

echo "🎉 Production deployment completed successfully!"
```

### Systemd Service Files

**neural-trading-platform.service**
```ini
[Unit]
Description=Neural Trading Platform
After=network.target
Wants=neural-trading-mcp-server.service

[Service]
Type=simple
User=trading
Group=trading
WorkingDirectory=/opt/neural-trading-platform/current
ExecStart=/opt/neural-trading-platform/current/trading-platform
ExecReload=/bin/kill -HUP $MAINPID
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal
SyslogIdentifier=neural-trading-platform

# Security settings
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=yes
ReadWritePaths=/opt/neural-trading-platform

# Environment
Environment=RUST_LOG=info
EnvironmentFile=/opt/neural-trading-platform/current/.env

# Resource limits
LimitNOFILE=65536
MemoryMax=2G

[Install]
WantedBy=multi-user.target
```

**neural-trading-mcp-server.service**
```ini
[Unit]
Description=Neural Trading MCP Server
After=network.target

[Service]
Type=simple
User=trading
Group=trading
WorkingDirectory=/opt/neural-trading-platform/current
ExecStart=/opt/neural-trading-platform/current/mcp-server
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=neural-trading-mcp

# Environment
EnvironmentFile=/opt/neural-trading-platform/current/.env

[Install]
WantedBy=multi-user.target
```

## Monitoring and Observability

### Health Checks

```bash
#!/bin/bash
# scripts/health-check.sh

set -e

echo "Performing health checks..."

# Check main service
if curl -f http://localhost:8081/health >/dev/null 2>&1; then
    echo "✅ Trading platform API is healthy"
else
    echo "❌ Trading platform API is down"
    exit 1
fi

# Check MCP server
if curl -f http://localhost:8080/health >/dev/null 2>&1; then
    echo "✅ MCP server is healthy"
else
    echo "❌ MCP server is down"
    exit 1
fi

# Check database
if cargo run --bin check-db-health >/dev/null 2>&1; then
    echo "✅ Database is healthy"
else
    echo "❌ Database connection failed"
    exit 1
fi

# Check agent status
AGENT_STATUS=$(curl -s http://localhost:8081/api/v1/agents | jq -r '.agents[] | select(.status != "active") | .id')
if [ -z "$AGENT_STATUS" ]; then
    echo "✅ All agents are active"
else
    echo "❌ Some agents are not active: $AGENT_STATUS"
    exit 1
fi

echo "All health checks passed!"
```

### Monitoring Setup

```bash
#!/bin/bash
# scripts/setup-monitoring.sh

set -e

echo "Setting up monitoring..."

# Start Prometheus
docker run -d \
    --name neural-trading-prometheus \
    --restart unless-stopped \
    -p 9090:9090 \
    -v $(pwd)/docker/monitoring/prometheus/prometheus.yml:/etc/prometheus/prometheus.yml \
    -v prometheus_data:/prometheus \
    prom/prometheus:latest

# Start Grafana
docker run -d \
    --name neural-trading-grafana \
    --restart unless-stopped \
    -p 3000:3000 \
    -e GF_SECURITY_ADMIN_PASSWORD=admin \
    -v grafana_data:/var/lib/grafana \
    -v $(pwd)/docker/monitoring/grafana/dashboards:/etc/grafana/provisioning/dashboards \
    grafana/grafana:latest

echo "Monitoring setup complete!"
echo "Grafana: http://localhost:3000 (admin/admin)"
echo "Prometheus: http://localhost:9090"
```

## Security Hardening

### SSL/TLS Configuration

```bash
#!/bin/bash
# scripts/setup-ssl.sh

set -e

echo "Setting up SSL certificates..."

# Generate self-signed certificate for development
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes \
    -subj "/C=US/ST=State/L=City/O=Organization/CN=localhost"

# Set proper permissions
chmod 600 key.pem
chmod 644 cert.pem

# Move to secure location
sudo mkdir -p /etc/neural-trading/ssl
sudo mv key.pem cert.pem /etc/neural-trading/ssl/
sudo chown root:trading /etc/neural-trading/ssl/*
sudo chmod 640 /etc/neural-trading/ssl/*

echo "SSL certificates installed"
```

### Firewall Configuration

```bash
#!/bin/bash
# scripts/configure-firewall.sh

set -e

echo "Configuring firewall..."

# Enable UFW
sudo ufw --force enable

# Default policies
sudo ufw default deny incoming
sudo ufw default allow outgoing

# Allow SSH (adjust port as needed)
sudo ufw allow 22/tcp

# Allow application ports
sudo ufw allow 8080/tcp comment "MCP Server"
sudo ufw allow 8081/tcp comment "Trading API"

# Allow monitoring (restrict to local network if needed)
sudo ufw allow from 192.168.0.0/16 to any port 3000 comment "Grafana"
sudo ufw allow from 192.168.0.0/16 to any port 9090 comment "Prometheus"

# Reload firewall
sudo ufw reload

echo "Firewall configured"
```

## Rollback Procedures

### Automatic Rollback

```bash
#!/bin/bash
# scripts/rollback.sh

set -e

DEPLOYMENT_DIR="/opt/neural-trading-platform"
BACKUP_DIR=$(ls -td $DEPLOYMENT_DIR/backup-* | head -1)

if [ -z "$BACKUP_DIR" ]; then
    echo "❌ No backup found for rollback"
    exit 1
fi

echo "Rolling back to: $BACKUP_DIR"

# Stop services
sudo systemctl stop neural-trading-platform
sudo systemctl stop neural-trading-mcp-server

# Rollback deployment
sudo mv $DEPLOYMENT_DIR/current $DEPLOYMENT_DIR/failed-$(date +%Y%m%d_%H%M%S)
sudo mv $BACKUP_DIR $DEPLOYMENT_DIR/current

# Start services
sudo systemctl start neural-trading-platform
sudo systemctl start neural-trading-mcp-server

# Verify rollback
sleep 10
if sudo systemctl is-active --quiet neural-trading-platform; then
    echo "✅ Rollback successful"
else
    echo "❌ Rollback failed"
    sudo journalctl -u neural-trading-platform --no-pager -n 20
    exit 1
fi
```

## Maintenance Procedures

### Scheduled Maintenance

```bash
#!/bin/bash
# scripts/maintenance.sh

set -e

echo "Starting scheduled maintenance..."

# Backup database
./scripts/backup-database.sh

# Clean old logs
find /opt/neural-trading-platform/current/logs -name "*.log" -mtime +7 -delete

# Update neural models (if enabled)
if [ "$AUTO_MODEL_UPDATE" = "true" ]; then
    cargo run --bin retrain-models
fi

# Health check after maintenance
./scripts/health-check.sh

echo "Scheduled maintenance completed"
```

### Update Procedure

```bash
#!/bin/bash
# scripts/update.sh

set -e

VERSION=$1
if [ -z "$VERSION" ]; then
    echo "Usage: $0 <version>"
    exit 1
fi

echo "Updating to version: $VERSION"

# Download new version
wget "https://releases.example.com/neural-trading-platform-${VERSION}.tar.gz"

# Verify checksum
wget "https://releases.example.com/neural-trading-platform-${VERSION}.tar.gz.sha256"
sha256sum -c neural-trading-platform-${VERSION}.tar.gz.sha256

# Deploy new version
mv neural-trading-platform-${VERSION}.tar.gz dist/
./scripts/deploy-production.sh

echo "Update completed to version: $VERSION"
```

## Troubleshooting

### Common Issues

#### Service Won't Start
```bash
# Check service status
sudo systemctl status neural-trading-platform

# Check logs
sudo journalctl -u neural-trading-platform -f

# Check configuration
./scripts/validate-config.sh

# Check permissions
ls -la /opt/neural-trading-platform/current/
```

#### Database Connection Issues
```bash
# Test database connection
cargo run --bin test-db-connection

# Check database logs
docker logs neural-trading-timescaledb-prod

# Verify database is running
docker ps | grep timescaledb
```

#### High Memory Usage
```bash
# Check memory usage
ps aux | grep trading-platform
free -h

# Restart services to clear memory
sudo systemctl restart neural-trading-platform
```

### Log Analysis

```bash
#!/bin/bash
# scripts/analyze-logs.sh

# Show recent errors
sudo journalctl -u neural-trading-platform --since "1 hour ago" | grep ERROR

# Show performance metrics
curl -s http://localhost:9091/metrics | grep -E "(latency|memory|cpu)"

# Show agent performance
curl -s http://localhost:8081/api/v1/agents | jq '.agents[] | {id: .id, performance: .performance}'
```

This deployment guide provides comprehensive procedures for deploying the Neural Trading Platform in production while maintaining security, reliability, and observability.