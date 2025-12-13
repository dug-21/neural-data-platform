# Neural Trader V2 Troubleshooting Guide

## Table of Contents

1. [Common Issues](#common-issues)
2. [Service-Specific Problems](#service-specific-problems)
3. [Performance Issues](#performance-issues)
4. [Configuration Problems](#configuration-problems)
5. [Development Environment](#development-environment)
6. [CI/CD Pipeline](#cicd-pipeline)
7. [Debugging Tools](#debugging-tools)
8. [Getting Help](#getting-help)

---

## Common Issues

### 1. Services Won't Start

**Symptoms:**
- Services exit immediately after starting
- Docker containers show "Exited" status
- Connection refused errors

**Solutions:**

```bash
# Check service logs
./scripts/v2/dev-logs.sh <service-name>

# Verify dependencies are running
docker-compose -f docker-compose.v2.yml ps

# Check port availability
netstat -tuln | grep -E "500[5-9][0-4]"

# Restart with clean state
./scripts/v2/dev-down.sh
docker-compose -f docker-compose.v2.yml rm -f
./scripts/v2/dev-up.sh
```

**Common Causes:**
- Port conflicts
- Missing environment variables
- Database not initialized
- Redis not running

### 2. Database Connection Errors

**Symptoms:**
- "connection refused" errors
- "database does not exist" messages
- Timeout errors

**Solutions:**

```bash
# Check TimescaleDB status
docker-compose -f docker-compose.v2.yml ps timescaledb

# Reinitialize database
docker-compose -f docker-compose.v2.yml exec timescaledb psql -U postgres -c "CREATE DATABASE neural_trader_v2;"
psql -h localhost -U postgres -d neural_trader_v2 -f scripts/v2/init-db.sql

# Test connection
PGPASSWORD=postgres psql -h localhost -U postgres -d neural_trader_v2 -c "SELECT 1;"
```

### 3. Redis Stream Issues

**Symptoms:**
- Messages not flowing between services
- "Stream does not exist" errors
- Consumer group errors

**Solutions:**

```bash
# Check Redis connectivity
redis-cli ping

# List all streams
redis-cli KEYS "*"

# Create missing streams
redis-cli XADD market-data "*" init "true"
redis-cli XGROUP CREATE market-data data-staging 0 MKSTREAM

# Check stream info
redis-cli XINFO STREAM market-data
```

---

## Service-Specific Problems

### Config-Store Issues

**Problem: Config-store can't clone repository**

```bash
# Check Git credentials
git ls-remote https://github.com/your-org/neural-trader-configs.git

# Verify environment variable
echo $CONFIG_REPO_URL

# Check file permissions
ls -la /tmp/config-cache/

# Manual clone test
git clone $CONFIG_REPO_URL /tmp/test-clone
```

**Problem: Config validation failures**

```bash
# Validate YAML syntax
python -m yaml configs/base/data-ingestion/config.yaml

# Check schema
jsonschema -i <(yq -o json configs/base/data-ingestion/config.yaml) configs/schemas/data-ingestion.schema.json
```

### Data-Ingestion Issues

**Problem: No data being ingested**

```bash
# Check API keys
echo $POLYGON_API_KEY

# Test synthetic data mode
curl -X POST http://localhost:8081/api/v1/control/synthetic/start

# Monitor Redis stream
redis-cli XREAD COUNT 10 STREAMS market-data $
```

### Data-Staging Issues

**Problem: Processing lag or failures**

```bash
# Check consumer group lag
redis-cli XPENDING market-data data-staging

# Reset consumer group
redis-cli XGROUP SETID market-data data-staging 0

# Check database writes
psql -h localhost -U postgres -d neural_trader_v2 -c "SELECT COUNT(*) FROM staging.processed_data WHERE created_at > NOW() - INTERVAL '5 minutes';"
```

---

## Performance Issues

### High Memory Usage

**Diagnosis:**

```bash
# Check container memory
docker stats --no-stream

# Find memory leaks
docker-compose -f docker-compose.v2.yml exec <service> top

# Check Rust service memory
cargo build --release
valgrind --leak-check=full target/release/<binary>
```

**Solutions:**
- Increase memory limits in docker-compose.yml
- Enable memory profiling
- Check for unbounded queues
- Review batch sizes

### Slow Processing

**Diagnosis:**

```bash
# Run performance test
./scripts/v2/test-pipeline.sh

# Check latency metrics
./scripts/v2/baseline-metrics.sh

# Monitor throughput
redis-cli --stat
```

**Solutions:**
- Increase parallel workers
- Optimize batch sizes
- Enable caching
- Use connection pooling

---

## Configuration Problems

### Environment Variable Issues

**Check all variables:**

```bash
# List current environment
env | grep -E "NEURAL|REDIS|DB|CONFIG"

# Source environment file
source .env

# Validate required variables
./scripts/v2/check-env.sh
```

### Config File Problems

**Validate configurations:**

```bash
# Check YAML syntax
for file in configs/**/*.yaml; do
    echo "Checking $file"
    python -m yaml "$file" || echo "ERROR in $file"
done

# Validate against schemas
./scripts/v2/config-validator.sh
```

---

## Development Environment

### Docker Issues

**Problem: Docker daemon not running**

```bash
# Linux
sudo systemctl start docker

# macOS
open -a Docker

# Check status
docker info
```

**Problem: Out of disk space**

```bash
# Clean up Docker
docker system prune -a --volumes

# Remove unused images
docker image prune -a

# Clear build cache
docker builder prune
```

### Rust Build Failures

**Problem: Compilation errors**

```bash
# Clean build
cargo clean
cargo build --release

# Update dependencies
cargo update

# Check for breaking changes
cargo tree
```

### Python Environment Issues

**Problem: Package conflicts**

```bash
# Recreate virtual environment
rm -rf venv/
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
```

---

## CI/CD Pipeline

### Pipeline Failures

**Module pipeline timeout:**

```bash
# Check which stage failed
cat /tmp/module-pipeline-report.html

# Run specific stage
./scripts/v2/module-test.sh data-ingestion

# Skip caching for fresh build
SKIP_CACHE=true ./scripts/v2/module-build.sh data-ingestion
```

**Platform pipeline errors:**

```bash
# Run with verbose logging
VERBOSE=true ./scripts/v2/run-pipeline.sh platform

# Check individual service logs
docker-compose -f docker-compose.v2.yml logs <service>
```

### Drift Detection Failures

**False positives:**

```bash
# Re-establish baseline
./scripts/v2/baseline-metrics.sh

# Adjust thresholds
vi configs/drift-thresholds.yaml

# Run specific drift test
./scripts/v2/drift-detection-tests.sh --test memory
```

---

## Debugging Tools

### Service Debugging

```bash
# Attach to running container
docker exec -it <container> /bin/sh

# Enable debug logging
export RUST_LOG=debug
export LOG_LEVEL=debug

# Use debugger
rust-gdb target/debug/<binary>
```

### Network Debugging

```bash
# Check service connectivity
nc -zv localhost 50051

# Test gRPC endpoint
grpcurl -plaintext localhost:50051 list

# Monitor network traffic
tcpdump -i any -n port 50051
```

### Log Analysis

```bash
# Aggregate logs
docker-compose -f docker-compose.v2.yml logs > all-logs.txt

# Search for errors
grep -E "ERROR|PANIC|FATAL" all-logs.txt

# Follow specific service
docker-compose -f docker-compose.v2.yml logs -f --tail=100 data-ingestion
```

---

## Getting Help

### Quick Diagnostics

Run the diagnostic script for a comprehensive health check:

```bash
./scripts/v2/diagnostics.sh
```

### Support Channels

1. **GitHub Issues**: [https://github.com/your-org/neural-trader/issues](https://github.com/your-org/neural-trader/issues)
2. **Slack Channel**: #neural-trader-support
3. **Documentation**: [/docs](./docs)
4. **Wiki**: [https://github.com/your-org/neural-trader/wiki](https://github.com/your-org/neural-trader/wiki)

### Information to Provide

When reporting issues, include:

1. **Environment details:**
   ```bash
   uname -a
   docker version
   cargo --version
   python --version
   ```

2. **Error messages:**
   - Full error output
   - Service logs
   - Stack traces

3. **Steps to reproduce:**
   - Commands run
   - Configuration used
   - Expected vs actual behavior

4. **Diagnostic output:**
   ```bash
   ./scripts/v2/diagnostics.sh > diagnostic-report.txt
   ```

---

## Emergency Procedures

### Complete System Reset

```bash
# Stop everything
./scripts/v2/dev-down.sh

# Clean up
docker system prune -a --volumes
rm -rf /tmp/config-cache
redis-cli FLUSHALL

# Reinitialize
./scripts/v2/setup-dev.sh
./scripts/v2/dev-up.sh
```

### Data Recovery

```bash
# Backup current state
pg_dump -h localhost -U postgres neural_trader_v2 > backup.sql
redis-cli BGSAVE

# Restore from backup
psql -h localhost -U postgres -d neural_trader_v2 < backup.sql
```

### Rollback Deployment

```bash
# Revert to previous version
git checkout <previous-tag>
make v2-build
./scripts/v2/dev-restart.sh
```

---

## Prevention Tips

1. **Regular Maintenance:**
   - Run drift detection daily
   - Monitor disk space
   - Update dependencies weekly

2. **Best Practices:**
   - Always check logs first
   - Use version tags for deployments
   - Keep backups of configurations
   - Document custom changes

3. **Monitoring:**
   - Set up alerts for critical errors
   - Monitor resource usage
   - Track performance metrics
   - Review logs regularly

---

*Last Updated: 2025-08-27*  
*Version: 1.0.0*