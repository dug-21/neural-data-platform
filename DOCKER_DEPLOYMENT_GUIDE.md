# Docker Deployment Guide - Neural Trader Platform

## 🚀 Overview

This guide provides comprehensive instructions for deploying the Neural Trader platform across development, testing, and production environments using Docker.

## 📋 Prerequisites

- Docker Engine 20.10+ 
- Docker Compose 2.0+
- 8GB+ RAM for development, 16GB+ for production
- SSL certificates for production (instructions included)

## 🔧 Quick Start

### Development Environment
```bash
# 1. Clone and setup
git clone <repository>
cd neural-trader

# 2. Create environment file
cp .env.example .env
# Edit .env with your API keys

# 3. Start development stack
docker-compose -f docker-compose.dev.yml up -d

# 4. Access services
# Neural Trader API: http://localhost:3030
# Redis Commander: http://localhost:8081
# pgAdmin: http://localhost:8082
```

### Production Environment
```bash
# 1. Generate secrets
mkdir -p secrets && chmod 700 secrets
openssl rand -base64 32 > secrets/postgres_password.txt
openssl rand -base64 32 > secrets/redis_password.txt
openssl rand -base64 64 > secrets/jwt_secret.txt

# 2. Generate SSL certificates
mkdir -p docker/nginx/ssl
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
  -keyout docker/nginx/ssl/key.pem \
  -out docker/nginx/ssl/cert.pem

# 3. Initialize Docker Swarm
docker swarm init

# 4. Deploy production stack
docker stack deploy -c docker-compose.prod.yml neural-trader
```

## 🌍 Environment Configurations

### Development (`docker-compose.dev.yml`)
**Purpose**: Local development with debugging tools
- ✅ Hot reload for code changes
- ✅ Debug tools (Redis Commander, pgAdmin)
- ✅ Exposed ports for direct access
- ✅ Simplified networking
- ✅ Local file mounts for development

**Services**:
- Neural Trader API (port 3030)
- TimescaleDB (port 5432)
- Redis (port 6379)
- Data Ingestion Service
- Redis Commander (port 8081)
- pgAdmin (port 8082)

### Testing (`docker-compose.test.yml`)
**Purpose**: Automated testing and CI/CD
- ✅ Isolated test environment
- ✅ In-memory databases (tmpfs)
- ✅ Comprehensive test suites
- ✅ Coverage reporting
- ✅ Performance benchmarking

**Test Types**:
- Unit tests (Rust + Python)
- Integration tests
- Performance benchmarks
- Security tests

### Production (`docker-compose.prod.yml`)
**Purpose**: Production deployment with full security
- ✅ Docker Swarm orchestration
- ✅ SSL/TLS termination
- ✅ Network isolation
- ✅ Secrets management
- ✅ Resource limits
- ✅ Health checks
- ✅ Backup automation
- ✅ Monitoring stack

**Services**:
- Neural Trader API (2 replicas)
- TimescaleDB (production config)
- Redis (production config)
- Data Ingestion Service
- Nginx (reverse proxy)
- Prometheus (metrics)
- Grafana (dashboards)
- Backup Service

## 🔐 Security Configuration

### Secrets Management
```bash
# Generate secure passwords
openssl rand -base64 32 > secrets/postgres_password.txt
openssl rand -base64 32 > secrets/redis_password.txt
openssl rand -base64 64 > secrets/jwt_secret.txt
openssl rand -base64 32 > secrets/grafana_password.txt

# API keys (from your providers)
echo "your_alpha_vantage_key" > secrets/alpha_vantage_api_key.txt
echo "your_polygon_key" > secrets/polygon_api_key.txt
echo "your_finnhub_key" > secrets/finnhub_api_key.txt
echo "your_iex_key" > secrets/iex_cloud_api_key.txt

# Set proper permissions
chmod 600 secrets/*.txt
```

### SSL Certificate Setup
```bash
# Self-signed (development)
mkdir -p docker/nginx/ssl
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
  -keyout docker/nginx/ssl/key.pem \
  -out docker/nginx/ssl/cert.pem \
  -subj "/C=US/ST=State/L=City/O=Organization/CN=localhost"

# Let's Encrypt (production)
certbot certonly --standalone -d yourdomain.com
cp /etc/letsencrypt/live/yourdomain.com/fullchain.pem docker/nginx/ssl/cert.pem
cp /etc/letsencrypt/live/yourdomain.com/privkey.pem docker/nginx/ssl/key.pem
```

### Network Security
- **Frontend Network**: Public-facing services (Nginx, Grafana)
- **Backend Network**: Internal services (API, databases)
- **Monitoring Network**: Metrics and monitoring
- **Internal-only**: Databases isolated from external access

## 🚀 Deployment Procedures

### Development Deployment
```bash
# Start development environment
docker-compose -f docker-compose.dev.yml up -d

# View logs
docker-compose -f docker-compose.dev.yml logs -f

# Stop environment
docker-compose -f docker-compose.dev.yml down
```

### Testing Deployment
```bash
# Run all tests
docker-compose -f docker-compose.test.yml up --abort-on-container-exit

# Run specific test suite
docker-compose -f docker-compose.test.yml run neural-trader-test cargo test
docker-compose -f docker-compose.test.yml run data-ingestion-test pytest

# View test results
docker-compose -f docker-compose.test.yml run integration-test
```

### Production Deployment
```bash
# Initialize swarm (if not done)
docker swarm init

# Deploy stack
docker stack deploy -c docker-compose.prod.yml neural-trader

# Monitor deployment
docker stack services neural-trader
docker service logs neural-trader_neural-trader

# Update services
docker service update --image neural-trader:latest neural-trader_neural-trader

# Scale services
docker service scale neural-trader_neural-trader=3
```

## 📊 Monitoring and Maintenance

### Health Checks
```bash
# Check service health
docker-compose -f docker-compose.prod.yml ps

# Check individual service health
curl -f http://localhost:3030/health
curl -f http://localhost:8001/health
```

### Monitoring Access
- **Prometheus**: http://localhost:9090
- **Grafana**: http://localhost:3000
- **Nginx Status**: http://localhost/health

### Backup Procedures
```bash
# Manual backup
docker-compose -f docker-compose.prod.yml exec backup /backup.sh

# Restore from backup
docker-compose -f docker-compose.prod.yml exec backup /restore.sh <backup_file>
```

## 🔧 Troubleshooting

### Common Issues

1. **Port Conflicts**
   ```bash
   # Check ports in use
   netstat -tulpn | grep :3030
   
   # Use different ports
   docker-compose -f docker-compose.dev.yml up -d --scale neural-trader=0
   ```

2. **Permission Errors**
   ```bash
   # Fix secrets permissions
   chmod 600 secrets/*.txt
   chown root:root secrets/*.txt
   ```

3. **Database Connection Issues**
   ```bash
   # Check database logs
   docker-compose logs timescaledb
   
   # Test connection
   docker-compose exec timescaledb psql -U postgres -d neural_trader
   ```

4. **Redis Connection Issues**
   ```bash
   # Check Redis logs
   docker-compose logs redis
   
   # Test connection
   docker-compose exec redis redis-cli ping
   ```

### Performance Optimization

1. **Resource Limits**
   ```yaml
   # Adjust in docker-compose.prod.yml
   resources:
     limits:
       cpus: '4'
       memory: 8G
     reservations:
       cpus: '2'
       memory: 4G
   ```

2. **Database Tuning**
   ```bash
   # Edit docker/timescaledb/postgresql.conf
   shared_buffers = 4GB
   effective_cache_size = 12GB
   ```

## 🚦 Environment Management

### Development to Production Workflow
1. Develop locally with `docker-compose.dev.yml`
2. Test with `docker-compose.test.yml`
3. Deploy to staging (production config with test data)
4. Deploy to production with `docker-compose.prod.yml`

### Configuration Management
- **Environment Variables**: Use `.env` files for each environment
- **Secrets**: Use Docker secrets in production
- **Configuration Files**: Mount configuration files as volumes
- **Feature Flags**: Use environment variables for feature toggles

## 📈 Scaling Strategies

### Horizontal Scaling
```bash
# Scale API service
docker service scale neural-trader_neural-trader=5

# Scale data ingestion
docker service scale neural-trader_data-ingestion=3
```

### Vertical Scaling
```bash
# Update resource limits
docker service update --limit-cpu 4 --limit-memory 8G neural-trader_neural-trader
```

### Database Scaling
- **Read Replicas**: Configure TimescaleDB read replicas
- **Connection Pooling**: Use PgBouncer for connection pooling
- **Partitioning**: Implement time-based partitioning

## 🛡️ Security Best Practices

1. **Never commit secrets to version control**
2. **Use Docker secrets for production**
3. **Regular security updates**
4. **Network segmentation**
5. **Regular backups**
6. **Log monitoring**
7. **SSL/TLS everywhere**

## 📚 Additional Resources

- [Docker Swarm Documentation](https://docs.docker.com/engine/swarm/)
- [TimescaleDB Docker Guide](https://docs.timescale.com/self-hosted/latest/install/installation-docker/)
- [Redis Docker Guide](https://redis.io/docs/getting-started/install-stack/docker/)
- [Nginx Docker Guide](https://docs.nginx.com/nginx/admin-guide/installing-nginx/installing-nginx-docker/)

## 🔧 Maintenance Schedule

### Daily
- Check service health
- Monitor resource usage
- Review logs for errors

### Weekly
- Review backup integrity
- Update security patches
- Performance monitoring review

### Monthly
- Security audit
- Capacity planning
- Dependency updates

---

**Support**: For issues or questions, create an issue in the repository or contact the development team.