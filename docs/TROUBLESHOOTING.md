# Neural Trader Platform Troubleshooting Guide

## Overview

This guide provides solutions to common issues encountered when using the Neural Trader Autonomous Platform. Issues are organized by category for quick reference.

## Table of Contents

- [Installation and Setup Issues](#installation-and-setup-issues)
- [Compilation and Build Issues](#compilation-and-build-issues)
- [Runtime and Configuration Issues](#runtime-and-configuration-issues)
- [Database Connection Issues](#database-connection-issues)
- [Neural Network and Model Issues](#neural-network-and-model-issues)
- [Performance and Memory Issues](#performance-and-memory-issues)
- [Network and API Issues](#network-and-api-issues)
- [Docker and Container Issues](#docker-and-container-issues)
- [Logging and Monitoring Issues](#logging-and-monitoring-issues)
- [Common Error Messages](#common-error-messages)

## Installation and Setup Issues

### Issue: Rust toolchain not found or outdated

**Symptoms:**
```bash
error: toolchain 'stable-x86_64-unknown-linux-gnu' is not installed
```

**Solution:**
```bash
# Install or update Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Update existing installation
rustup update
```

### Issue: Missing system dependencies

**Symptoms:**
```bash
error: linking with `cc` failed
note: /usr/bin/ld: cannot find -lpq: No such file or directory
```

**Solutions:**

**Ubuntu/Debian:**
```bash
sudo apt update
sudo apt install build-essential pkg-config libpq-dev libssl-dev
```

**CentOS/RHEL:**
```bash
sudo yum groupinstall "Development Tools"
sudo yum install postgresql-devel openssl-devel
```

**macOS:**
```bash
# Install Xcode command line tools
xcode-select --install

# Install dependencies with Homebrew
brew install postgresql openssl
```

### Issue: Docker not installed or not running

**Symptoms:**
```bash
Cannot connect to the Docker daemon at unix:///var/run/docker.sock
```

**Solution:**
```bash
# Install Docker (Ubuntu/Debian)
curl -fsSL https://get.docker.com | sh
sudo usermod -aG docker $USER

# Start Docker service
sudo systemctl start docker
sudo systemctl enable docker

# Log out and back in for group changes to take effect
```

## Compilation and Build Issues

### Issue: Cargo build fails with dependency conflicts

**Symptoms:**
```bash
error: failed to select a version for the requirement `tokio = "^1.0"`
```

**Solution:**
```bash
# Clean cargo cache and rebuild
cargo clean
rm -rf Cargo.lock

# Update dependencies
cargo update
cargo build
```

### Issue: Out of memory during compilation

**Symptoms:**
```bash
error: linking with `cc` failed: exit status: 1
note: collect2: fatal error: ld terminated with signal 9 [Killed]
```

**Solution:**
```bash
# Reduce parallel compilation jobs
export CARGO_BUILD_JOBS=1
cargo build --release

# Or use incremental compilation
export CARGO_INCREMENTAL=1
cargo build
```

### Issue: Feature flag conflicts

**Symptoms:**
```bash
error[E0433]: failed to resolve: maybe a missing crate `feature`?
```

**Solution:**
```bash
# Check Cargo.toml for conflicting features
# Build with specific features
cargo build --no-default-features --features "minimal"

# Or build with all features
cargo build --all-features
```

## Runtime and Configuration Issues

### Issue: Configuration file not found

**Symptoms:**
```bash
Error: Failed to read config file: "config/platform.toml"
```

**Solution:**
```bash
# Ensure config directory exists
mkdir -p config

# Copy example configuration
cp config/platform.toml.example config/platform.toml

# Or create minimal config
cat > config/platform.toml << EOF
[platform]
name = "neural-trader"
version = "0.1.0"

[database]
url = "postgres://neural_trader:password@localhost/neural_trader_db"
max_connections = 20

[redis]
url = "redis://localhost:6379"
max_connections = 10

[neural]
memory_gb = 1.0
models = ["MLP"]
EOF
```

### Issue: Environment variable override not working

**Symptoms:**
Configuration values don't change despite setting environment variables.

**Solution:**
```bash
# Ensure correct environment variable format
export DATABASE_URL="postgres://user:pass@localhost/db"
export NEURAL_MEMORY_GB=2.0

# Check that environment variables are set
env | grep -E "(DATABASE|NEURAL|REDIS|MONITORING)"

# Restart the application after setting variables
```

### Issue: Invalid configuration values

**Symptoms:**
```bash
Error: Configuration validation failed: max_connections cannot exceed 100
```

**Solution:**
1. Review configuration limits in `src/config.rs`
2. Adjust values to be within valid ranges:
   ```toml
   [database]
   max_connections = 50  # Must be reasonable for your system
   min_connections = 5   # Must be less than max_connections
   
   [neural]
   memory_gb = 4.0       # Must be positive
   models = ["NHITS"]    # Must contain at least one model
   
   [monitoring]
   quality_threshold = 0.95  # Must be between 0.0 and 1.0
   ```

## Database Connection Issues

### Issue: PostgreSQL connection refused

**Symptoms:**
```bash
Error: connection to server at "localhost" (127.0.0.1), port 5432 failed: Connection refused
```

**Solution:**
```bash
# Check if PostgreSQL is running
sudo systemctl status postgresql

# Start PostgreSQL if not running
sudo systemctl start postgresql

# Check if Docker containers are running
docker-compose ps

# Start database containers
docker-compose up -d postgres
```

### Issue: Authentication failed for PostgreSQL

**Symptoms:**
```bash
Error: FATAL: password authentication failed for user "neural_trader"
```

**Solution:**
```bash
# Check database credentials in config
# Ensure user exists and has correct password
docker-compose exec postgres psql -U postgres -c "
CREATE USER neural_trader WITH PASSWORD 'neural_trader_pass';
CREATE DATABASE neural_trader_db OWNER neural_trader;
GRANT ALL PRIVILEGES ON DATABASE neural_trader_db TO neural_trader;"

# Or reset password
docker-compose exec postgres psql -U postgres -c "
ALTER USER neural_trader PASSWORD 'new_password';"
```

### Issue: TimescaleDB extension not available

**Symptoms:**
```bash
Error: extension "timescaledb" is not available
```

**Solution:**
```bash
# Use TimescaleDB Docker image
# Update docker-compose.yml:
services:
  postgres:
    image: timescale/timescaledb:latest-pg13
    environment:
      POSTGRES_DB: neural_trader_db
      POSTGRES_USER: neural_trader
      POSTGRES_PASSWORD: neural_trader_pass

# Restart containers
docker-compose down
docker-compose up -d
```

### Issue: Database migration or schema errors

**Symptoms:**
```bash
Error: relation "time_series_data" does not exist
```

**Solution:**
```bash
# Run database initialization
cargo run --bin init-db

# Or manually create tables
docker-compose exec postgres psql -U neural_trader -d neural_trader_db -f docker/init-db.sql
```

## Neural Network and Model Issues

### Issue: Model loading failures

**Symptoms:**
```bash
Error: Failed to load neural model: NHITS
```

**Solution:**
```bash
# Check if model files exist
ls models/

# Download or create model files
mkdir -p models
# Place your trained models in the models/ directory

# Use simpler models for testing
# Update config to use basic models:
[neural]
models = ["MLP"]  # Start with simple models
```

### Issue: Out of memory during neural operations

**Symptoms:**
```bash
Error: CUDA out of memory
# or
thread 'main' panicked at 'allocation of ... bytes failed'
```

**Solution:**
```bash
# Reduce memory allocation in config
[neural]
memory_gb = 1.0  # Reduce from higher values
max_batch_size = 100  # Reduce batch size
thread_pool_size = 2  # Reduce thread count

# Disable GPU if causing issues
[neural]
gpu_memory_fraction = 0.0  # Disable GPU usage
```

### Issue: Model prediction timeouts

**Symptoms:**
```bash
Error: Model operation timed out after 30 seconds
```

**Solution:**
```bash
# Increase timeout in configuration
[neural]
model_timeout_secs = 60  # Increase from 30

# Or reduce model complexity
[neural]
max_batch_size = 50  # Process smaller batches
```

## Performance and Memory Issues

### Issue: High memory usage

**Symptoms:**
System becomes slow or out of memory errors occur.

**Diagnosis:**
```bash
# Monitor memory usage
htop
# or
ps aux --sort=-%mem | head -10

# Check application metrics
curl http://localhost:8080/metrics | grep memory
```

**Solution:**
```bash
# Reduce memory-intensive operations
[neural]
memory_gb = 0.5  # Reduce neural model memory
max_batch_size = 100

[database]
max_connections = 10  # Reduce connection pool

[redis]
max_connections = 5
```

### Issue: High CPU usage

**Symptoms:**
```bash
CPU usage consistently above 80%
```

**Solution:**
```bash
# Reduce CPU-intensive operations
[neural]
thread_pool_size = 2  # Reduce thread count

[monitoring]
metrics_interval_secs = 300  # Reduce monitoring frequency

# Check for infinite loops in logs
tail -f logs/app.log | grep ERROR
```

### Issue: Memory leaks

**Symptoms:**
Memory usage increases over time without bound.

**Diagnosis:**
```bash
# Monitor memory growth
watch -n 10 'ps -eo pid,ppid,cmd,%mem,%cpu --sort=-%mem | head'

# Check for long-running operations
curl http://localhost:8080/metrics | grep -E "(active|connections|operations)"
```

**Solution:**
```bash
# Restart the application periodically
# Add to crontab:
0 2 * * * systemctl restart neural-trader

# Or investigate specific memory leaks
# Enable debug logging and monitor patterns
```

## Network and API Issues

### Issue: API endpoints not responding

**Symptoms:**
```bash
curl: (7) Failed to connect to localhost port 8080: Connection refused
```

**Solution:**
```bash
# Check if application is running
systemctl status neural-trader
# or
ps aux | grep neural-trader

# Check port binding
netstat -tulpn | grep 8080
# or
ss -tulpn | grep 8080

# Check firewall settings
sudo ufw status
sudo iptables -L
```

### Issue: Slow API responses

**Symptoms:**
API responses take > 5 seconds.

**Diagnosis:**
```bash
# Test API response times
time curl http://localhost:8080/health

# Check application logs
tail -f logs/app.log | grep -E "(slow|timeout|latency)"
```

**Solution:**
```bash
# Optimize configuration
[database]
max_connections = 50  # Increase connection pool

[redis]
response_timeout_ms = 1000  # Reduce timeout

# Enable caching
[neural]
prediction_cache_ttl = 300  # Cache predictions
```

### Issue: Rate limiting errors

**Symptoms:**
```bash
HTTP 429 Too Many Requests
```

**Solution:**
```bash
# Adjust rate limiting configuration
[security]
rate_limit_requests_per_minute = 1000  # Increase limit
rate_limit_burst = 50

# Or disable rate limiting for testing
[security]
enable_rate_limiting = false
```

## Docker and Container Issues

### Issue: Container startup failures

**Symptoms:**
```bash
ERROR: Container exited with code 125
```

**Solution:**
```bash
# Check container logs
docker-compose logs neural-trader
docker-compose logs postgres
docker-compose logs redis

# Restart specific services
docker-compose restart postgres
docker-compose restart neural-trader
```

### Issue: Port binding conflicts

**Symptoms:**
```bash
ERROR: Port 5432 is already in use
```

**Solution:**
```bash
# Check what's using the port
lsof -i :5432
netstat -tulpn | grep 5432

# Stop conflicting service
sudo systemctl stop postgresql

# Or change port in docker-compose.yml
ports:
  - "5433:5432"  # Use different host port
```

### Issue: Volume mount issues

**Symptoms:**
```bash
ERROR: Cannot mount volume: permission denied
```

**Solution:**
```bash
# Fix permissions
sudo chown -R $USER:$USER ./data
chmod 755 ./data

# Or use Docker-managed volumes
# Update docker-compose.yml:
volumes:
  postgres_data:  # Use named volume instead of bind mount
```

### Issue: Network connectivity between containers

**Symptoms:**
Containers can't communicate with each other.

**Solution:**
```bash
# Check Docker network
docker network ls
docker network inspect neural-trader_default

# Ensure containers are on same network
# Use service names for internal communication:
DATABASE_URL=postgres://user:pass@postgres:5432/db
REDIS_URL=redis://redis:6379
```

## Logging and Monitoring Issues

### Issue: No logs being generated

**Symptoms:**
Log files are empty or don't exist.

**Solution:**
```bash
# Check log configuration
[logging]
level = "info"  # Ensure appropriate log level
output = "file"
file_path = "/var/log/neural-trader/app.log"

# Ensure log directory exists and is writable
sudo mkdir -p /var/log/neural-trader
sudo chown neural-trader:neural-trader /var/log/neural-trader

# Check RUST_LOG environment variable
export RUST_LOG=info
```

### Issue: Metrics not being exported

**Symptoms:**
Prometheus metrics endpoint returns 404.

**Solution:**
```bash
# Check metrics configuration
[monitoring]
prometheus_port = 8080
prometheus_path = "/metrics"

# Test metrics endpoint
curl http://localhost:8080/metrics

# Check if metrics are enabled
[monitoring]
enable_metrics = true
```

### Issue: Log rotation not working

**Symptoms:**
Log files grow very large.

**Solution:**
```bash
# Configure logrotate
sudo tee /etc/logrotate.d/neural-trader << EOF
/var/log/neural-trader/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
    create 644 neural-trader neural-trader
    postrotate
        systemctl reload neural-trader
    endscript
}
EOF

# Test logrotate
sudo logrotate -d /etc/logrotate.d/neural-trader
```

## Common Error Messages

### "Failed to bind to address"

**Cause:** Port already in use or insufficient permissions.

**Solution:**
```bash
# Check what's using the port
sudo lsof -i :8080

# Change port in configuration
[platform]
port = 8081

# Or kill conflicting process
sudo kill -9 <PID>
```

### "Connection pool exhausted"

**Cause:** Too many database connections or connection leaks.

**Solution:**
```bash
# Increase connection pool size
[database]
max_connections = 50

# Check for connection leaks in code
# Monitor active connections
curl http://localhost:8080/metrics | grep db_connections
```

### "Model inference timeout"

**Cause:** Neural model operations taking too long.

**Solution:**
```bash
# Increase timeout
[neural]
model_timeout_secs = 60

# Reduce batch size
max_batch_size = 50

# Use simpler models
models = ["MLP"]
```

### "Cache operation failed"

**Cause:** Redis connection issues or memory limits.

**Solution:**
```bash
# Check Redis status
docker-compose logs redis

# Increase Redis memory
# In docker-compose.yml:
command: redis-server --maxmemory 256mb --maxmemory-policy allkeys-lru

# Check Redis connectivity
redis-cli -h localhost -p 6379 ping
```

## Getting Additional Help

### Enable Debug Logging

```bash
# Increase logging verbosity
export RUST_LOG=debug

# Or in configuration
[logging]
level = "debug"

# Restart application and check logs
tail -f logs/app.log
```

### Collect System Information

```bash
# Create diagnostic report
mkdir -p debug-info

# System information
uname -a > debug-info/system.txt
lscpu > debug-info/cpu.txt
free -h > debug-info/memory.txt
df -h > debug-info/disk.txt

# Application information
cargo --version > debug-info/cargo.txt
rustc --version > debug-info/rust.txt
docker --version > debug-info/docker.txt

# Configuration
cp config/platform.toml debug-info/
cp docker-compose.yml debug-info/

# Logs
tail -1000 logs/app.log > debug-info/app.log

# Create archive
tar -czf neural-trader-debug.tar.gz debug-info/
```

### Performance Profiling

```bash
# CPU profiling
perf record -g cargo run --release
perf report

# Memory profiling with valgrind
valgrind --tool=memcheck --leak-check=full ./target/release/neural-trader

# Application profiling
cargo install flamegraph
cargo flamegraph --bin neural-trader
```

### Health Checks

```bash
# Basic health check
curl http://localhost:8080/health

# Component-specific checks
curl http://localhost:8080/health/database
curl http://localhost:8080/health/redis
curl http://localhost:8080/health/neural

# Metrics overview
curl http://localhost:8080/metrics | grep -E "(up|error|latency)"
```

## Prevention Best Practices

1. **Regular Updates:** Keep dependencies and system packages updated
2. **Monitoring:** Set up proper alerting for critical metrics
3. **Backups:** Regular database and configuration backups
4. **Testing:** Test configuration changes in staging environment
5. **Documentation:** Keep deployment and configuration notes updated
6. **Logging:** Maintain appropriate log levels for troubleshooting
7. **Resource Monitoring:** Monitor CPU, memory, and disk usage trends

For issues not covered in this guide, check the application logs, review the source code documentation, or create an issue in the project repository with detailed information about your environment and the problem.