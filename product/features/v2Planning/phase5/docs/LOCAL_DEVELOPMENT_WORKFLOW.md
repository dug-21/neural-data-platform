# Local Development Workflow Guide

## Quick Start - Module Development

```bash
# Working on a specific module? Test just that module!

# 1. Set environment
export CONFIG_ENV=dev

# 2. Test single module (3 min)
make pipeline MODULE=neural-trading

# 3. Test multiple related modules (5 min)
make pipeline MODULES="data-staging neural-ml-ops"

# 4. Full platform test when ready (16 min)
make pipeline
```

## Quick Start - Full Platform

```bash
# Need to test everything? Use platform mode

# 1. Set environment
export CONFIG_ENV=dev

# 2. Start infrastructure
make dev-infra

# 3. Start config-store (must be first)
make dev-config-store

# 4. Start all services
make dev-services

# 5. Run full test suite
make platform-pipeline
```

## Development Environment Setup

### Prerequisites
```bash
# Required tools
- Docker & Docker Compose
- Rust toolchain (rustc 1.75+)
- Python 3.11+
- Make
- Git

# Check prerequisites
make check-prerequisites
```

### Initial Setup
```bash
# Clone repository
git clone https://github.com/org/neural-trader.git
cd neural-trader

# Setup development environment
./scripts/setup-dev.sh

# Create .env.dev file (copy from template)
cp .env.dev.template .env.dev
# Edit .env.dev with your API keys (Alpaca, etc.)
```

## Service Development Workflow

### 1. Module-First Development (NEW - Faster!)

Focus on the module you're actively developing:

```bash
# Example: Working on neural-trading module

# Quick test cycle (30 seconds)
make module-test MODULE=neural-trading

# Test with minimal dependencies (2 min)
make module-integration MODULE=neural-trading

# Full module pipeline (3 min)
make pipeline MODULE=neural-trading
```

### 2. Config-Store First Principle

For modules that need config-store (most do):

```bash
# Module pipeline handles this automatically!
make pipeline MODULE=neural-trading  # Sets up config-store for you

# Or manually if needed:
docker-compose -f docker-compose.v2.yml up -d config-store
docker-compose -f docker-compose.v2.yml ps config-store
```

### 3. Individual Module Development

#### Rust Module Development
```bash
# Work on specific module
cd neural-trading/

# Quick unit test (20 sec)
cargo test -p neural-trading

# Module pipeline (3 min)
make pipeline MODULE=neural-trading

# Debug with minimal services
make module-integration MODULE=neural-trading KEEP_ALIVE=true
docker-compose -f docker-compose.v2.yml exec neural-trading bash
```

#### Python Module Development
```bash
# Work on data-ingestion
cd data_ingestion/

# Quick unit test (15 sec)
pytest tests/unit/

# Module pipeline (2 min)
make pipeline MODULE=data-ingestion

# Debug mode
KEEP_ALIVE=true make module-integration MODULE=data-ingestion
```

### 4. Cross-Module Development

When changes affect multiple modules:

```bash
# Test affected modules together (5 min)
make pipeline MODULES="neural-core data-staging neural-trading"

# Or test sequentially with shared infrastructure
make module-setup MODULE=data-staging
make module-test MODULE=data-staging
make module-test MODULE=neural-trading  # Reuses running services
make module-teardown
```

### 5. Full Platform Development

Only when you need to validate everything:

```bash
# Complete platform test (16 min)
make platform-pipeline

# Platform without regression (12 min)
make platform-pipeline SKIP_REGRESSION=true

# Watch all logs during platform test
make platform-integration &
docker-compose -f docker-compose.v2.yml logs -f
```

## Configuration Management

### GitOps Workflow

```bash
# 1. Edit configuration
vim config/dev/services/neural-trading.yaml

# 2. Commit changes
git add config/dev/
git commit -m "Update neural-trading dev config"

# 3. Restart config-store to reload
docker-compose -f docker-compose.v2.yml restart config-store

# 4. Services auto-reload config (future feature)
# For now, restart service
docker-compose -f docker-compose.v2.yml restart neural-trading
```

### Testing Configuration Changes

```bash
# Test with different environment
CONFIG_ENV=test docker-compose -f docker-compose.v2.yml up -d

# Verify config loaded correctly
docker-compose -f docker-compose.v2.yml exec neural-trading \
  curl http://localhost:8080/config
```

## Testing Workflows

### Unit Testing (No Containers)
```bash
# Run all unit tests
make test-unit

# Run specific service tests
cd neural-trading && cargo test

# Run with coverage
make test-coverage
```

### Integration Testing (With Containers)
```bash
# Start test environment
CONFIG_ENV=test make test-setup

# Run integration tests
make test-integration

# Keep environment for debugging
KEEP_ALIVE=true make test-integration

# Clean up
make test-teardown
```

### Regression Testing
```bash
# Run regression suite (alert only)
make test-regression

# View drift report
cat drift-report.json | jq
```

## Debugging Workflows

### Service Debugging

```bash
# Attach to running container
docker-compose -f docker-compose.v2.yml exec neural-trading bash

# View service logs
docker-compose -f docker-compose.v2.yml logs neural-trading

# Follow logs in real-time
docker-compose -f docker-compose.v2.yml logs -f neural-trading

# Check service health
curl http://localhost:8080/health
```

### EventBus Debugging

```bash
# Monitor EventBus messages
docker-compose -f docker-compose.v2.yml exec redis \
  redis-cli XREAD BLOCK 0 STREAMS eventbus:trading:signals 0

# Check consumer groups
docker-compose -f docker-compose.v2.yml exec redis \
  redis-cli XINFO GROUPS eventbus:trading:signals
```

### Database Debugging

```bash
# Connect to TimescaleDB
docker-compose -f docker-compose.v2.yml exec timescaledb \
  psql -U postgres neural_trader

# Run queries
SELECT * FROM market_data WHERE symbol='AAPL' ORDER BY timestamp DESC LIMIT 10;
```

## Common Development Tasks

### Adding a New Service

```bash
# 1. Create service directory
cargo new services/new-service

# 2. Add to Cargo workspace
echo 'members = ["services/new-service"]' >> Cargo.toml

# 3. Create Dockerfile
cp docker/Dockerfile.template docker/Dockerfile.new-service

# 4. Add to docker-compose
vim docker-compose.v2.yml

# 5. Add configuration
vim config/dev/services/new-service.yaml

# 6. Update pipeline
vim Makefile
```

### Updating Dependencies

```bash
# Rust dependencies
cargo update

# Python dependencies
cd data_ingestion
pip-compile requirements.in

# Docker images
docker-compose -f docker-compose.v2.yml pull
```

### Running CICD Locally

```bash
# Full pipeline
make pipeline

# Specific stages
make build test-unit

# With custom environment
CONFIG_ENV=test make pipeline

# Verbose output
VERBOSE=true make pipeline
```

## Makefile Commands

### Essential Make Targets

```makefile
# Development
make dev-up          # Start full dev environment
make dev-down        # Stop everything
make dev-restart     # Restart all services
make dev-logs        # Show all logs

# Testing
make test            # Run all tests
make test-unit       # Unit tests only
make test-integration # Integration tests
make test-regression # Regression tests

# Building
make build           # Build all services
make build-rust      # Build Rust services
make build-python    # Build Python services
make build-docker    # Build Docker images

# Utilities
make clean           # Clean build artifacts
make fmt             # Format code
make lint            # Run linters
make check           # Run all checks
```

## Environment Variables

### Development (.env.dev)
```bash
# Config Store
CONFIG_REPO_URL=https://github.com/org/neural-trader.git
CONFIG_BRANCH=main
CONFIG_ENV=dev

# Secrets (git-ignored)
ALPACA_API_KEY=PKxxxxx
ALPACA_SECRET_KEY=xxxxxx
POSTGRES_PASSWORD=dev_password

# Service URLs
REDIS_URL=redis://localhost:6379
TIMESCALE_URL=postgresql://postgres:dev_password@localhost:5432/neural_trader
```

## Troubleshooting

### Common Issues

#### Config-Store Not Starting
```bash
# Check logs
docker-compose -f docker-compose.v2.yml logs config-store

# Verify Git access
git ls-remote $CONFIG_REPO_URL

# Check health
docker-compose -f docker-compose.v2.yml ps config-store
```

#### Service Can't Connect to Config-Store
```bash
# Verify config-store is running
docker-compose -f docker-compose.v2.yml ps config-store

# Check network
docker network ls
docker network inspect neural-trader_default

# Test connectivity
docker-compose -f docker-compose.v2.yml exec neural-trading \
  nc -zv config-store 50051
```

#### EventBus Not Receiving Messages
```bash
# Check Redis
docker-compose -f docker-compose.v2.yml exec redis redis-cli ping

# Monitor streams
docker-compose -f docker-compose.v2.yml exec redis \
  redis-cli MONITOR

# Check consumer groups
docker-compose -f docker-compose.v2.yml exec redis \
  redis-cli XINFO STREAM eventbus:trading:signals
```

## Best Practices

1. **Always Start Config-Store First**: Other services depend on it
2. **Use CONFIG_ENV**: Keeps environments separate
3. **Test Locally First**: Before pushing changes
4. **Check Logs**: When debugging issues
5. **Clean Rebuilds**: When dependencies change
6. **Incremental Changes**: Test as you go

## VS Code Integration

### tasks.json
```json
{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "Dev Up",
      "type": "shell",
      "command": "make dev-up"
    },
    {
      "label": "Run Tests",
      "type": "shell",
      "command": "make test"
    }
  ]
}
```

### launch.json
```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "type": "lldb",
      "request": "launch",
      "name": "Debug neural-trading",
      "program": "${workspaceFolder}/target/debug/neural-trading",
      "env": {
        "CONFIG_STORE_URL": "http://localhost:50051"
      }
    }
  ]
}
```

## Next Steps

1. Set up your local environment
2. Start config-store
3. Run a test service
4. Make a config change
5. Run the test suite