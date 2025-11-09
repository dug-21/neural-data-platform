# Claude Flow Agent Instructions for Neural Trader V2

## Overview

This document provides instructions for Claude Flow agents to work with the Neural Trader V2 CI/CD pipeline and development environment.

## Agent Capabilities Required

### Essential Agents
- **cicd-engineer**: CI/CD pipeline management
- **backend-dev**: Rust service development  
- **tester**: Testing and validation
- **repo-architect**: Repository structure optimization
- **docker-specialist**: Container orchestration

### Supporting Agents
- **performance-benchmarker**: Performance analysis
- **code-analyzer**: Code quality checks
- **api-docs**: Documentation generation
- **security-manager**: Security scanning

## Quick Start Commands

### Environment Setup

```bash
# Initialize development environment with parallel setup
./scripts/v2/setup-dev.sh

# Start all services
./scripts/v2/dev-up.sh

# Verify services are healthy
docker-compose -f docker-compose.v2.yml ps
```

### Running Pipelines

#### Module-Specific Pipeline (Target: <3 minutes)

```bash
# Run module pipeline with caching
make v2-module-pipeline MODULE=data-ingestion

# Run without cache for clean build
SKIP_CACHE=true make v2-module-pipeline MODULE=data-ingestion

# Run specific module test
make v2-test-module MODULE=data-staging
```

#### Platform-Wide Pipeline (Target: <16 minutes)

```bash
# Run full platform pipeline
make v2-platform-pipeline

# Run with performance monitoring
MONITOR=true make v2-platform-pipeline

# Generate comprehensive report
make v2-pipeline-report
```

## Service Management

### Individual Service Operations

```bash
# Rebuild specific service
make v2-build-module MODULE=config-store

# Restart service
./scripts/v2/dev-restart.sh data-ingestion

# View service logs
./scripts/v2/dev-logs.sh neural-trading

# Debug service
docker exec -it data-staging /bin/sh
```

### Batch Operations (Parallel)

```bash
# Build all services in parallel
make v2-build-parallel

# Test all modules concurrently
make v2-test-all-parallel

# Health check all services
make v2-health-check-all
```

## Configuration Management

### GitOps Workflow

```bash
# Seed configurations from Git
./scripts/v2/config-seeder.sh dev

# Validate configurations
./scripts/v2/config-validator.sh

# Apply configuration overlay
kubectl kustomize configs/overlays/dev | kubectl apply -f -
```

### Environment-Specific Configs

```bash
# Switch environment
export ENVIRONMENT=test
./scripts/v2/config-seeder.sh test

# Verify loaded configuration
grpcurl -plaintext localhost:50050 config.ConfigStore/GetConfig
```

## Testing Strategies

### Unit Testing

```bash
# Run unit tests for specific module
cd v2/data-ingestion && cargo test

# Run with coverage
cargo tarpaulin --out Html
```

### Integration Testing

```bash
# Test data pipeline flow
./scripts/v2/test-pipeline.sh

# Fix connection issues
./scripts/v2/fix-connection.sh

# Verify EventBus messaging
./scripts/v2/verify-eventbus.sh
```

### Performance Testing

```bash
# Establish baseline metrics
./scripts/v2/baseline-metrics.sh

# Run drift detection
./scripts/v2/drift-detection-tests.sh

# Load test with synthetic data
./scripts/v2/load-test.sh --duration 300 --rate 100
```

## Monitoring & Alerts

### Real-time Monitoring

```bash
# Start monitoring loop
./scripts/v2/alert-mechanisms.sh monitor

# Check current alerts
./scripts/v2/alert-mechanisms.sh check

# Generate alert summary
./scripts/v2/alert-mechanisms.sh summary
```

### Metrics Collection

```bash
# Collect performance metrics
./scripts/v2/collect-metrics.sh

# Export metrics for analysis
./scripts/v2/export-metrics.sh --format json > metrics.json

# Visualize metrics (requires local Grafana)
make v2-grafana-up
```

## Debugging Workflows

### Service Debugging

```bash
# Enable debug logging
export RUST_LOG=debug
export LOG_LEVEL=debug

# Attach debugger to service
rust-gdb target/debug/data-ingestion

# Profile memory usage
valgrind --leak-check=full target/release/data-staging
```

### Pipeline Debugging

```bash
# Run pipeline with verbose output
VERBOSE=true ./scripts/v2/run-pipeline.sh module data-ingestion

# Generate detailed failure report
./scripts/v2/generate-failure-report.sh

# Analyze bottlenecks
./scripts/v2/analyze-bottlenecks.sh
```

## Common Tasks for Agents

### 1. Add New Service

```bash
# Create service structure
mkdir -p v2/new-service/{src,tests}

# Add to docker-compose.yml
# Edit docker-compose.v2.yml

# Create Dockerfile
cp docker/Dockerfile.template docker/Dockerfile.new-service

# Update Makefile
# Add new-service to SERVICES list in Makefile.v2
```

### 2. Update Dependencies

```bash
# Update Rust dependencies
cargo update

# Update Python packages
pip install --upgrade -r requirements.txt

# Update Docker base images
docker-compose -f docker-compose.v2.yml pull
```

### 3. Fix Failing Tests

```bash
# Identify failing tests
make v2-test 2>&1 | grep FAILED

# Run specific failing test
cargo test test_name --exact

# Debug test with output
cargo test -- --nocapture
```

### 4. Optimize Performance

```bash
# Profile build times
time make v2-build-module MODULE=data-ingestion

# Enable parallel builds
export CARGO_BUILD_JOBS=8

# Use build cache
export DOCKER_BUILDKIT=1
```

## Agent Collaboration Patterns

### Sequential Workflow

```javascript
// 1. cicd-engineer initializes pipeline
await agent.spawn('cicd-engineer', {
  task: 'setup-pipeline',
  config: 'docker-compose.v2.yml'
});

// 2. backend-dev builds services
await agent.spawn('backend-dev', {
  task: 'build-services',
  parallel: true
});

// 3. tester validates
await agent.spawn('tester', {
  task: 'run-tests',
  coverage: true
});
```

### Parallel Workflow

```javascript
// Spawn multiple agents concurrently
await Promise.all([
  agent.spawn('code-analyzer', { task: 'analyze-code' }),
  agent.spawn('security-manager', { task: 'scan-vulnerabilities' }),
  agent.spawn('api-docs', { task: 'generate-docs' })
]);
```

## Best Practices

### 1. Always Use Parallel Processing

```bash
# Good: Parallel builds
make -j8 v2-build

# Bad: Sequential builds
for service in $SERVICES; do
  make v2-build-module MODULE=$service
done
```

### 2. Cache Aggressively

```bash
# Enable all caching layers
export CARGO_TARGET_DIR=/tmp/cargo-cache
export DOCKER_BUILDKIT=1
export BUILDKIT_INLINE_CACHE=1
```

### 3. Monitor Resource Usage

```bash
# Track resource consumption
docker stats --no-stream
htop
df -h
```

### 4. Document Changes

```bash
# Update documentation after changes
make v2-docs-update

# Generate API documentation
cargo doc --open
```

## Troubleshooting for Agents

### Common Issues

1. **Port conflicts**: Check with `netstat -tuln`
2. **Memory issues**: Increase Docker memory limit
3. **Build failures**: Clear cache with `cargo clean`
4. **Connection errors**: Verify network with `docker network ls`

### Recovery Procedures

```bash
# Full system reset
./scripts/v2/reset-all.sh

# Restore from backup
./scripts/v2/restore-backup.sh

# Rollback deployment
git checkout <previous-tag> && make v2-deploy
```

## Memory Storage Keys

Agents should store important information using these standardized keys:

```bash
# Store pipeline results
memory_store "neural-trader/pipeline/results" "$results"

# Store performance metrics
memory_store "neural-trader/metrics/performance" "$metrics"

# Store configuration state
memory_store "neural-trader/config/current" "$config"

# Store error logs
memory_store "neural-trader/errors/latest" "$errors"
```

## Integration with Claude Flow

### Swarm Initialization

```bash
npx claude-flow swarm init --topology mesh --maxAgents 8

# Spawn specialized agents
npx claude-flow agent spawn --type cicd-engineer
npx claude-flow agent spawn --type backend-dev
npx claude-flow agent spawn --type tester
```

### Task Orchestration

```bash
# Orchestrate pipeline task
npx claude-flow task orchestrate \
  --task "Run Neural Trader V2 CI/CD Pipeline" \
  --strategy parallel \
  --priority high

# Monitor progress
npx claude-flow swarm status
```

## Success Criteria

Agents should verify these criteria before marking tasks complete:

### Build Success
- [ ] All services compile without errors
- [ ] Docker images built successfully
- [ ] Build time < 3 minutes for modules

### Test Success
- [ ] Unit tests pass (100%)
- [ ] Integration tests pass (>95%)
- [ ] Coverage > 70%

### Deployment Success
- [ ] All services healthy
- [ ] Data flowing through pipeline
- [ ] No critical alerts

## Reporting

Generate comprehensive reports after pipeline execution:

```bash
# Generate HTML report
./scripts/v2/generate-report.sh --format html > report.html

# Generate JSON metrics
./scripts/v2/generate-report.sh --format json > metrics.json

# Generate Markdown summary
./scripts/v2/generate-report.sh --format markdown > summary.md
```

---

## Quick Reference Card

```bash
# Essential Commands
setup:    ./scripts/v2/setup-dev.sh
start:    ./scripts/v2/dev-up.sh
stop:     ./scripts/v2/dev-down.sh
logs:     ./scripts/v2/dev-logs.sh [service]
build:    make v2-build-module MODULE=<name>
test:     make v2-test-module MODULE=<name>
pipeline: ./scripts/v2/run-pipeline.sh [module|platform]
monitor:  ./scripts/v2/alert-mechanisms.sh monitor
```

---

*Last Updated: 2025-08-27*  
*Version: 1.0.0*  
*For: Claude Flow Agents working with Neural Trader V2*