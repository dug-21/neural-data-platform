# Docker Optimization Guide for Neural Trader

## 🎯 Overview

This guide provides optimized Docker strategies for running Neural Trader in resource-constrained environments like GitHub Codespaces.

## 🚨 The Problem

- **Limited disk space**: Codespaces typically provides 32GB total storage
- **Build overhead**: Rust builds can consume 5-10GB during compilation
- **Layer accumulation**: Multi-stage builds create intermediate layers
- **Cache growth**: Docker build cache grows without bounds

## 🚀 Solution Strategies

### 1. Hybrid Development Mode (Recommended)

Run databases in Docker, application locally:

```bash
./scripts/start_hybrid_development.sh
```

**Benefits:**
- Saves ~4GB by not building Rust in Docker
- Faster iteration with local hot reload
- Keeps database state persistent
- Easy debugging with native tools

### 2. Optimized Docker Builds

Use BuildKit and optimized Dockerfiles:

```bash
export DOCKER_BUILDKIT=1
./scripts/start_full_stock_simulation_optimized.sh
```

**Features:**
- BuildKit cache mounts
- Parallel builds
- Automatic cache pruning
- Resource limits

### 3. External Docker Host

For Codespaces, use the host Docker daemon:

```bash
./scripts/setup_external_docker.sh
# Select option 1 for Codespaces host
```

**Benefits:**
- Builds happen outside container
- No nested virtualization overhead
- Access to more resources

## 📊 Resource Management

### Disk Space Monitoring

```bash
# Check Docker usage
docker system df

# Check filesystem usage
df -h /

# Detailed container sizes
docker ps -s
```

### Cleanup Commands

```bash
# Quick cleanup (safe)
./scripts/docker_cleanup.sh
# Select option 1 or 2

# Remove all stopped containers
docker container prune -f

# Remove unused images
docker image prune -af

# Clean build cache (keep 1GB)
docker builder prune -f --keep-storage=1GB

# Nuclear option (removes everything)
docker system prune -af --volumes
```

## 🛠️ Configuration Options

### 1. docker-compose.optimized.yml

Optimized compose file with:
- Resource limits per service
- tmpfs mounts for temporary data
- Delegated volume mounts
- BuildKit caching

### 2. Dockerfile.optimized

Features:
- Cache mounts for package managers
- Separate dependency and code layers
- Minimal final images
- Build-time optimizations

### 3. Environment Variables

```bash
# Always set these
export DOCKER_BUILDKIT=1
export COMPOSE_DOCKER_CLI_BUILD=1

# For external Docker
export DOCKER_HOST=unix:///var/run/docker-host.sock

# For build limits
export DOCKER_BUILD_MEMORY=2g
export DOCKER_BUILD_CPUS=2
```

## 🎮 Usage Patterns

### Development Workflow

1. **Initial Setup**
   ```bash
   # First time setup
   ./scripts/setup_external_docker.sh
   source ~/.bashrc
   ```

2. **Daily Development**
   ```bash
   # Hybrid mode (recommended)
   ./scripts/start_hybrid_development.sh
   
   # Or full Docker mode
   ./scripts/start_full_stock_simulation_optimized.sh
   ```

3. **Before Major Builds**
   ```bash
   # Clean up space
   ./scripts/docker_cleanup.sh
   
   # Check available space
   df -h /
   ```

### Production Builds

```bash
# Use multi-stage build
docker build -f Dockerfile.optimized --target production -t neural-trader:prod .

# Or with buildx for caching
docker buildx build --cache-from type=registry,ref=myregistry/cache \
                   --cache-to type=inline \
                   -t neural-trader:prod .
```

## 🏷️ Best Practices

### 1. Use .dockerignore

```dockerfile
# .dockerignore
target/
.git/
*.log
.env*
docs/
tests/
*.md
```

### 2. Order Dockerfile Commands

```dockerfile
# Dependencies first (changes rarely)
COPY Cargo.toml Cargo.lock ./
RUN cargo build --release

# Code last (changes frequently)
COPY src ./src
RUN cargo build --release
```

### 3. Leverage Build Cache

```dockerfile
# Use cache mounts
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release
```

### 4. Set Resource Limits

```yaml
deploy:
  resources:
    limits:
      memory: 2G
      cpus: '2.0'
```

## 🚦 Troubleshooting

### Out of Space Errors

1. Check what's using space:
   ```bash
   docker system df
   docker images -a
   ```

2. Clean aggressively:
   ```bash
   docker system prune -af --volumes
   docker builder prune -af
   ```

3. Use external Docker:
   ```bash
   export DOCKER_HOST=unix:///var/run/docker-host.sock
   ```

### Slow Builds

1. Enable BuildKit:
   ```bash
   export DOCKER_BUILDKIT=1
   ```

2. Use parallel builds:
   ```bash
   docker-compose build --parallel
   ```

3. Increase builder resources:
   ```bash
   docker buildx create --use \
     --driver-opt env.BUILDKIT_STEP_LOG_MAX_SIZE=50000000
   ```

### Container Crashes

1. Check logs:
   ```bash
   docker-compose logs -f [service]
   ```

2. Increase memory limits:
   ```yaml
   deploy:
     resources:
       limits:
         memory: 4G
   ```

3. Use swap if needed:
   ```bash
   sudo sysctl vm.swappiness=60
   ```

## 📈 Performance Metrics

### Baseline (Unoptimized)
- Build time: ~15 minutes
- Disk usage: ~10GB
- Memory peak: 4GB

### Optimized
- Build time: ~5 minutes
- Disk usage: ~4GB
- Memory peak: 2GB

### Hybrid Mode
- Build time: ~2 minutes (local)
- Disk usage: ~1GB (Docker only)
- Memory peak: 1GB

## 🔗 Quick Reference

| Command | Purpose |
|---------|---------|
| `./scripts/start_hybrid_development.sh` | Best for development |
| `./scripts/start_full_stock_simulation_optimized.sh` | Full Docker stack |
| `./scripts/docker_cleanup.sh` | Free up space |
| `./scripts/setup_external_docker.sh` | Configure external Docker |
| `docker system df` | Check Docker disk usage |
| `docker system prune -af` | Nuclear cleanup |

## 📚 Additional Resources

- [Docker BuildKit Documentation](https://docs.docker.com/develop/develop-images/build_enhancements/)
- [Docker Compose Resource Constraints](https://docs.docker.com/compose/compose-file/compose-file-v3/#resources)
- [Dockerfile Best Practices](https://docs.docker.com/develop/develop-images/dockerfile_best-practices/)
- [GitHub Codespaces Docs](https://docs.github.com/en/codespaces)