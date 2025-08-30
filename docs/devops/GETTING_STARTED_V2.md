# Getting Started with Neural Trader V2 - Complete Setup Guide

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Initial Setup](#initial-setup)
3. [Configuration Setup](#configuration-setup)
4. [Database Setup](#database-setup)
5. [Service Build](#service-build)
6. [Environment Variables](#environment-variables)
7. [Running the System](#running-the-system)
8. [Verification](#verification)
9. [Common Issues](#common-issues)
10. [Next Steps](#next-steps)

---

## Prerequisites

### System Requirements

- **OS**: Linux, macOS, or Windows with WSL2
- **RAM**: Minimum 8GB (16GB recommended)
- **Disk Space**: 20GB free space
- **CPU**: 4+ cores recommended

### Required Software

```bash
# Check if installed
docker --version          # Docker 20.10+
docker-compose --version   # Docker Compose 2.0+
git --version             # Git 2.30+
make --version            # GNU Make 4.0+
python3 --version         # Python 3.9+
cargo --version           # Rust 1.70+ (optional for local builds)
```

### Install Missing Dependencies

#### macOS
```bash
# Install Homebrew if needed
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install dependencies
brew install docker docker-compose git make python3
brew install --cask docker  # For Docker Desktop

# Install Rust (optional)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

#### Ubuntu/Debian
```bash
# Update packages
sudo apt update

# Install dependencies
sudo apt install -y docker.io docker-compose git make python3 python3-pip curl

# Install Rust (optional)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add user to docker group
sudo usermod -aG docker $USER
newgrp docker
```

---

## Initial Setup

### 1. Clone the Repository

```bash
# Clone the repository
git clone https://github.com/your-org/neural-trader.git
cd neural-trader

# Verify you're on the correct branch
git checkout v2-phase5
```

### 2. Project Structure Verification

Ensure these directories exist:

```bash
# Required directories
neural-trader/
├── v2/                     # Microservices source code
│   ├── config-store/
│   ├── data-ingestion/
│   ├── data-staging/
│   ├── neural-ml-ops/
│   └── neural-trading/
├── docker/v2/              # Dockerfiles
├── configs/                # Configuration files
├── scripts/v2/             # Automation scripts
├── proto/                  # Protocol buffer definitions
└── tests/                  # Test suites
```

If missing, create the structure:

```bash
# Create missing directories
mkdir -p v2/{config-store,data-ingestion,data-staging,neural-ml-ops,neural-trading}
mkdir -p docker/v2 configs/{base,overlays,schemas} scripts/v2
mkdir -p proto tests/{unit,integration,synthetic}
mkdir -p logs metrics/{baseline,drift} data/synthetic
```

---

## Configuration Setup

### 1. Create Base Configurations

Each service needs a base configuration file:

```bash
# Create config directories for each service
for service in config-store data-ingestion data-staging neural-ml-ops neural-trading; do
    mkdir -p configs/base/$service
done
```

Create minimal configs if they don't exist:

```bash
# config-store configuration
cat > configs/base/config-store/config.yaml << 'EOF'
service:
  name: config-store
  version: 1.0.0
  
grpc:
  port: 50050
  host: 0.0.0.0
  
git:
  repo_url: "${CONFIG_REPO_URL}"
  branch: main
  poll_interval: 60
  
storage:
  type: local
  path: /tmp/config-cache
EOF

# data-ingestion configuration  
cat > configs/base/data-ingestion/config.yaml << 'EOF'
service:
  name: data-ingestion
  version: 1.0.0
  
api:
  port: 50051
  host: 0.0.0.0
  
redis:
  url: redis://redis:6379
  stream: market-data
  
sources:
  synthetic:
    enabled: true
    interval: 1
EOF

# Continue for other services...
```

### 2. Create Environment Overlays

```bash
# Development overlay
cat > configs/overlays/dev/kustomization.yaml << 'EOF'
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization

bases:
  - ../../base

configMapGenerator:
  - name: environment-config
    literals:
      - ENVIRONMENT=dev
      - LOG_LEVEL=debug
      - ENABLE_DEBUG=true
EOF
```

### 3. Create Docker Compose Configuration

If `docker-compose.v2.yml` doesn't exist:

```bash
cat > docker-compose.v2.yml << 'EOF'
version: '3.8'

services:
  # Infrastructure
  timescaledb:
    image: timescale/timescaledb:latest-pg15
    environment:
      POSTGRES_DB: neural_trader_v2
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: postgres
    ports:
      - "5432:5432"
    volumes:
      - timescale_data:/var/lib/postgresql/data
      - ./scripts/v2/init-db.sql:/docker-entrypoint-initdb.d/init.sql
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 10s
      timeout: 5s
      retries: 5

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis_data:/data
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 5s
      retries: 5

  # Services (will be added after build)
  config-store:
    build:
      context: .
      dockerfile: docker/v2/Dockerfile.config-store
    ports:
      - "50050:50050"
    environment:
      - CONFIG_REPO_URL=${CONFIG_REPO_URL:-local}
      - RUST_LOG=info
    depends_on:
      - redis
    volumes:
      - ./configs:/configs

  data-ingestion:
    build:
      context: .
      dockerfile: docker/v2/Dockerfile.data-ingestion
    ports:
      - "50051:50051"
    environment:
      - REDIS_URL=redis://redis:6379
      - RUST_LOG=info
    depends_on:
      - redis
      - config-store

  data-staging:
    build:
      context: .
      dockerfile: docker/v2/Dockerfile.data-staging
    ports:
      - "50052:50052"
    environment:
      - REDIS_URL=redis://redis:6379
      - DATABASE_URL=postgresql://postgres:postgres@timescaledb:5432/neural_trader_v2
      - RUST_LOG=info
    depends_on:
      - timescaledb
      - redis
      - config-store

  neural-ml-ops:
    build:
      context: .
      dockerfile: docker/v2/Dockerfile.neural-ml-ops
    ports:
      - "50053:50053"
    environment:
      - DATABASE_URL=postgresql://postgres:postgres@timescaledb:5432/neural_trader_v2
      - RUST_LOG=info
    depends_on:
      - timescaledb
      - data-staging

  neural-trading:
    build:
      context: .
      dockerfile: docker/v2/Dockerfile.neural-trading
    ports:
      - "50054:50054"
      - "8080:8080"
    environment:
      - RUST_LOG=info
    depends_on:
      - neural-ml-ops

volumes:
  timescale_data:
  redis_data:

networks:
  default:
    name: neural-trader-v2
EOF
```

---

## Database Setup

### 1. Create Database Initialization Script

```bash
cat > scripts/v2/init-db.sql << 'EOF'
-- Create Neural Trader V2 Database
CREATE DATABASE IF NOT EXISTS neural_trader_v2;

\c neural_trader_v2;

-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- Create schemas
CREATE SCHEMA IF NOT EXISTS market;
CREATE SCHEMA IF NOT EXISTS staging;
CREATE SCHEMA IF NOT EXISTS ml;
CREATE SCHEMA IF NOT EXISTS trading;
CREATE SCHEMA IF NOT EXISTS config;

-- Market data tables
CREATE TABLE IF NOT EXISTS market.market_data (
    time TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(10) NOT NULL,
    price DECIMAL(10, 2),
    volume BIGINT,
    bid DECIMAL(10, 2),
    ask DECIMAL(10, 2)
);

-- Convert to hypertable
SELECT create_hypertable('market.market_data', 'time', 
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE);

-- Create indexes
CREATE INDEX idx_market_data_symbol_time ON market.market_data (symbol, time DESC);

-- Add more tables as needed...
EOF
```

### 2. Initialize Database

```bash
# Start only the database first
docker-compose -f docker-compose.v2.yml up -d timescaledb redis

# Wait for database to be ready
sleep 10

# Initialize database (if init script not auto-run)
PGPASSWORD=postgres psql -h localhost -U postgres -f scripts/v2/init-db.sql

# Verify database
PGPASSWORD=postgres psql -h localhost -U postgres -d neural_trader_v2 -c "\dt market.*"
```

---

## Service Build

### 1. Create Minimal Service Stubs

If services don't exist yet, create minimal Rust projects:

```bash
# Create each service
for service in config-store data-ingestion data-staging neural-ml-ops neural-trading; do
    if [ ! -f "v2/$service/Cargo.toml" ]; then
        cd v2/$service
        cargo init --name $service
        cd ../..
    fi
done
```

### 2. Create Dockerfiles

```bash
# Create a base Dockerfile template
cat > docker/v2/Dockerfile.template << 'EOF'
FROM rust:1.70-alpine AS builder
WORKDIR /app
COPY v2/SERVICE_NAME/Cargo.toml .
COPY v2/SERVICE_NAME/src ./src
RUN cargo build --release

FROM alpine:3.18
RUN apk add --no-cache ca-certificates
COPY --from=builder /app/target/release/SERVICE_NAME /usr/local/bin/
CMD ["SERVICE_NAME"]
EOF

# Create service-specific Dockerfiles
for service in config-store data-ingestion data-staging neural-ml-ops neural-trading; do
    sed "s/SERVICE_NAME/$service/g" docker/v2/Dockerfile.template > docker/v2/Dockerfile.$service
done
```

### 3. Create Makefile

```bash
cat > Makefile.v2 << 'EOF'
.PHONY: help build test clean

SERVICES := config-store data-ingestion data-staging neural-ml-ops neural-trading

help:
	@echo "Neural Trader V2 Makefile"
	@echo "Available targets:"
	@echo "  v2-build         - Build all services"
	@echo "  v2-test          - Run all tests"
	@echo "  v2-up            - Start all services"
	@echo "  v2-down          - Stop all services"
	@echo "  v2-clean         - Clean build artifacts"

v2-build:
	@for service in $(SERVICES); do \
		echo "Building $$service..."; \
		docker-compose -f docker-compose.v2.yml build $$service; \
	done

v2-up:
	docker-compose -f docker-compose.v2.yml up -d

v2-down:
	docker-compose -f docker-compose.v2.yml down

v2-logs:
	docker-compose -f docker-compose.v2.yml logs -f

v2-clean:
	docker-compose -f docker-compose.v2.yml down -v
	rm -rf target/
EOF
```

---

## Environment Variables

### 1. Create Environment File

```bash
cat > .env << 'EOF'
# Neural Trader V2 Environment Configuration

# Environment
ENVIRONMENT=dev

# Database
DB_HOST=localhost
DB_PORT=5432
DB_NAME=neural_trader_v2
DB_USER=postgres
DB_PASSWORD=postgres
DATABASE_URL=postgresql://postgres:postgres@localhost:5432/neural_trader_v2

# Redis
REDIS_URL=redis://localhost:6379
REDIS_HOST=localhost
REDIS_PORT=6379

# Service Ports
CONFIG_STORE_PORT=50050
DATA_INGESTION_PORT=50051
DATA_STAGING_PORT=50052
NEURAL_ML_OPS_PORT=50053
NEURAL_TRADING_PORT=50054

# Config Management
CONFIG_REPO_URL=https://github.com/your-org/neural-trader-configs.git
CONFIG_BRANCH=main

# Logging
RUST_LOG=info
LOG_LEVEL=info

# API Keys (use test keys for development)
POLYGON_API_KEY=test_key
ALPHA_VANTAGE_API_KEY=test_key

# Performance
PARALLEL_JOBS=4
CACHE_ENABLED=true
EOF
```

### 2. Source Environment

```bash
# Load environment variables
source .env

# Verify
echo "Database URL: $DATABASE_URL"
echo "Redis URL: $REDIS_URL"
```

---

## Running the System

### 1. Automated Setup (Recommended)

```bash
# Run the complete setup script
./scripts/v2/setup-dev.sh

# This will:
# - Check prerequisites
# - Install dependencies
# - Create directories
# - Initialize database
# - Build services
# - Create helper scripts
```

### 2. Manual Setup Steps

```bash
# Step 1: Start infrastructure
docker-compose -f docker-compose.v2.yml up -d timescaledb redis

# Step 2: Wait for infrastructure
sleep 10

# Step 3: Initialize database
PGPASSWORD=postgres psql -h localhost -U postgres -f scripts/v2/init-db.sql

# Step 4: Build services
make -f Makefile.v2 v2-build

# Step 5: Start all services
make -f Makefile.v2 v2-up

# Step 6: Verify services
docker-compose -f docker-compose.v2.yml ps
```

---

## Verification

### 1. Check Service Health

```bash
# Check all services are running
docker-compose -f docker-compose.v2.yml ps

# Expected output:
# NAME                 STATUS    PORTS
# timescaledb         Up        0.0.0.0:5432->5432/tcp
# redis               Up        0.0.0.0:6379->6379/tcp
# config-store        Up        0.0.0.0:50050->50050/tcp
# data-ingestion      Up        0.0.0.0:50051->50051/tcp
# data-staging        Up        0.0.0.0:50052->50052/tcp
# neural-ml-ops       Up        0.0.0.0:50053->50053/tcp
# neural-trading      Up        0.0.0.0:50054->50054/tcp
```

### 2. Test Connectivity

```bash
# Test database
PGPASSWORD=postgres psql -h localhost -U postgres -d neural_trader_v2 -c "SELECT 1"

# Test Redis
redis-cli ping
# Expected: PONG

# Test gRPC services (requires grpcurl)
grpcurl -plaintext localhost:50050 list
grpcurl -plaintext localhost:50051 list
```

### 3. Check Logs

```bash
# View all logs
./scripts/v2/dev-logs.sh

# View specific service logs
./scripts/v2/dev-logs.sh data-ingestion

# Check for errors
docker-compose -f docker-compose.v2.yml logs | grep ERROR
```

### 4. Run Integration Test

```bash
# Test data flow
./scripts/v2/test-pipeline.sh

# Check connection between services
./scripts/v2/fix-connection.sh
```

---

## Common Issues

### Issue 1: Services Won't Start

```bash
# Check ports are available
netstat -tuln | grep -E "500[5-9][0-4]|5432|6379"

# Kill conflicting processes
sudo lsof -ti:50051 | xargs kill -9

# Restart with clean state
docker-compose -f docker-compose.v2.yml down -v
docker-compose -f docker-compose.v2.yml up -d
```

### Issue 2: Database Connection Failed

```bash
# Ensure database is running
docker-compose -f docker-compose.v2.yml up -d timescaledb

# Check database logs
docker-compose -f docker-compose.v2.yml logs timescaledb

# Recreate database
docker-compose -f docker-compose.v2.yml exec timescaledb psql -U postgres -c "DROP DATABASE IF EXISTS neural_trader_v2"
docker-compose -f docker-compose.v2.yml exec timescaledb psql -U postgres -c "CREATE DATABASE neural_trader_v2"
```

### Issue 3: Build Failures

```bash
# Clear Docker cache
docker system prune -a

# Rebuild without cache
docker-compose -f docker-compose.v2.yml build --no-cache

# Check Dockerfile syntax
docker build -f docker/v2/Dockerfile.data-ingestion .
```

### Issue 4: Config Not Loading

```bash
# Check config files exist
ls -la configs/base/*/config.yaml

# Validate YAML syntax
python3 -m yaml configs/base/data-ingestion/config.yaml

# Check environment variables
env | grep CONFIG
```

---

## Next Steps

### 1. Run the Full Pipeline

```bash
# Execute complete pipeline with monitoring
./scripts/v2/full-pipeline-execution.sh

# Check performance against targets
# Module pipeline: <3 minutes
# Platform pipeline: <16 minutes
```

### 2. Set Up Monitoring

```bash
# Start monitoring
./scripts/v2/alert-mechanisms.sh monitor

# Set up baseline metrics
./scripts/v2/baseline-metrics.sh
```

### 3. Configure IDE

```bash
# Open in VS Code with configurations
code . 

# Install recommended extensions
# Use provided tasks.json and launch.json
```

### 4. Run Security Scan

```bash
# Perform security audit
./scripts/v2/security-scan.sh

# Review security report
cat security-reports/security_summary_*.txt
```

### 5. Explore Documentation

- [Architecture Overview](../architecture/)
- [API Documentation](../api/)
- [Troubleshooting Guide](./TROUBLESHOOTING_GUIDE.md)
- [Claude Flow Instructions](./CLAUDE_FLOW_INSTRUCTIONS.md)

---

## Quick Command Reference

```bash
# Start everything
./scripts/v2/dev-up.sh

# Stop everything  
./scripts/v2/dev-down.sh

# View logs
./scripts/v2/dev-logs.sh [service-name]

# Restart service
./scripts/v2/dev-restart.sh <service-name>

# Run tests
make -f Makefile.v2 v2-test

# Build specific service
make -f Makefile.v2 v2-build-module MODULE=data-ingestion

# Run pipeline
./scripts/v2/run-pipeline.sh module data-ingestion
```

---

## Support

If you encounter issues not covered here:

1. Check the [Troubleshooting Guide](./TROUBLESHOOTING_GUIDE.md)
2. Review service logs: `./scripts/v2/dev-logs.sh`
3. Run diagnostics: `./scripts/v2/diagnostics.sh`
4. Contact support: #neural-trader-support on Slack

---

*Last Updated: 2025-08-27*  
*Version: 1.0.0*  
*For: Neural Trader V2 Initial Setup*