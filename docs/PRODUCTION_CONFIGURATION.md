# Production Configuration and Observability Guide

## Overview

This document describes the comprehensive production-ready configuration and observability system implemented for the Neural Trader Autonomous Platform. The system provides robust configuration management, real-time monitoring, structured logging, and distributed tracing capabilities.

## Configuration Architecture

### Environment-Specific Configurations

The system supports multiple environment configurations:

- **`config/production.toml`**: Production-optimized settings with enhanced security and monitoring
- **`config/development.toml`**: Development-friendly configuration with debug features
- **`config/platform.toml`**: Base configuration template

### Configuration Structure

```toml
[platform]
name = "neural-trader-autonomous"
version = "0.1.0"
environment = "production"
log_level = "info"

[database]
url = "postgres://user:pass@prod-db:5432/neural_trader"
max_connections = 50
connection_timeout = 30
idle_timeout = 600
max_query_time = 60

[redis]
url = "redis://prod-redis:6379"
max_connections = 25
default_ttl_seconds = 7200
connection_timeout_ms = 5000
cluster_mode = false

[neural]
memory_gb = 4.0
models = ["NHITS", "DeepAR", "TCN", "MLP", "Transformer"]
prediction_cache_ttl = 1800
model_load_timeout = 300
max_concurrent_predictions = 50
enable_model_monitoring = true
accuracy_threshold = 0.85

[monitoring]
metrics_interval_secs = 30
quality_threshold = 0.95
enable_performance_metrics = true
enable_memory_monitoring = true
enable_error_monitoring = true
cpu_usage_threshold = 80.0
memory_usage_threshold = 85.0
error_rate_threshold = 0.05
prometheus_port = 8080
prometheus_path = "/metrics"

[security]
enable_tls = true
tls_cert_path = "/etc/ssl/certs/neural-trader.crt"
tls_key_path = "/etc/ssl/private/neural-trader.key"
rate_limit_requests_per_minute = 1000
rate_limit_burst = 100
request_timeout_seconds = 30
enable_cors = true
allowed_origins = ["https://neural-trader.com"]

[performance]
worker_threads = 8
async_queue_size = 10000
enable_tcp_keepalive = true
tcp_keepalive_time = 7200
tcp_keepalive_interval = 75
tcp_keepalive_probes = 9

[logging]
level = "info"
format = "json"
enable_file_logging = true
log_file_path = "/var/log/neural-trader/app.log"
log_file_max_size_mb = 100
log_file_max_files = 10
async_logging = true
filter_sensitive_data = true

[alerts]
enable_email_alerts = true
email_smtp_server = "smtp.gmail.com"
email_smtp_port = 587
email_from = "alerts@neural-trader.com"
email_to = ["admin@neural-trader.com"]
enable_slack_alerts = true
slack_webhook_url = "https://hooks.slack.com/services/YOUR/SLACK/WEBHOOK"

[backup]
enable_automatic_backup = true
backup_interval_hours = 6
backup_retention_days = 30
backup_storage_path = "/var/backups/neural-trader"
enable_cloud_backup = true
cloud_backup_provider = "s3"
cloud_backup_bucket = "neural-trader-backups"

[circuit_breaker]
enable_circuit_breaker = true
failure_threshold = 5
recovery_timeout_seconds = 60
half_open_max_calls = 10

[graceful_shutdown]
shutdown_timeout_seconds = 30
enable_graceful_shutdown = true
drain_timeout_seconds = 10
```

## Environment Variable Overrides

All configuration values can be overridden using environment variables with the format `SECTION_FIELD_NAME`:

### Database Configuration
```bash
export DATABASE_URL="postgres://user:pass@localhost:5432/neural_trader"
export DATABASE_MAX_CONNECTIONS=50
export DATABASE_MIN_CONNECTIONS=10
export DATABASE_CONNECTION_TIMEOUT=30
export DATABASE_IDLE_TIMEOUT=600
export DATABASE_MAX_QUERY_TIME=60
```

### Redis Configuration
```bash
export REDIS_URL="redis://localhost:6379"
export REDIS_MAX_CONNECTIONS=25
export REDIS_DEFAULT_TTL_SECONDS=7200
export REDIS_CONNECTION_TIMEOUT_MS=5000
export REDIS_CLUSTER_MODE=false
```

### Neural Network Configuration
```bash
export NEURAL_MEMORY_GB=4.0
export NEURAL_MODELS="NHITS,DeepAR,TCN,MLP,Transformer"
export NEURAL_PREDICTION_CACHE_TTL=1800
export NEURAL_MODEL_LOAD_TIMEOUT=300
export NEURAL_MAX_CONCURRENT_PREDICTIONS=50
export NEURAL_ACCURACY_THRESHOLD=0.85
```

### Monitoring Configuration
```bash
export MONITORING_METRICS_INTERVAL_SECS=30
export MONITORING_PROMETHEUS_PORT=8080
export MONITORING_CPU_USAGE_THRESHOLD=80.0
export MONITORING_MEMORY_USAGE_THRESHOLD=85.0
export MONITORING_ERROR_RATE_THRESHOLD=0.05
```

### Security Configuration
```bash
export SECURITY_ENABLE_TLS=true
export SECURITY_TLS_CERT_PATH="/etc/ssl/certs/neural-trader.crt"
export SECURITY_TLS_KEY_PATH="/etc/ssl/private/neural-trader.key"
export SECURITY_RATE_LIMIT_REQUESTS_PER_MINUTE=1000
export SECURITY_ALLOWED_ORIGINS="https://neural-trader.com,https://api.neural-trader.com"
```

## Usage Examples

### Loading Configuration

```rust
use neural_trader::config::{load_config_for_environment, load_production_config};

// Load configuration based on environment
let config = load_config_for_environment("production")?;

// Load specific configuration
let production_config = load_production_config()?;
let development_config = load_development_config()?;
```

### Configuration Validation

The system automatically validates all configuration values:

```rust
use neural_trader::config::PlatformConfig;

let config = PlatformConfig::load("config/production.toml")?;
// Validation happens automatically during load
// Will return error if any validation fails
```

## Observability System

### Structured Logging

The system provides comprehensive structured logging with sensitive data filtering:

```rust
use neural_trader::observability::logger::{LogLevel, BusinessLogger, SecurityLogger};

// Business event logging
BusinessLogger::log_prediction("NHITS", 0.156, 0.92, 150);
BusinessLogger::log_trading_decision("AAPL", "BUY", 100.0, 150.25, "High confidence prediction");

// Security event logging
SecurityLogger::log_authentication("user123", true, "192.168.1.100");
SecurityLogger::log_authorization("user123", "/api/v1/trade", "POST", true);
```

### Metrics Collection

#### Business Metrics
```rust
use neural_trader::observability::metrics::MetricsRegistry;

let metrics = MetricsRegistry::new();

// Record prediction metrics
metrics.business().record_prediction("NHITS", inference_duration, 0.92);

// Record trading metrics
metrics.business().record_trade(true, 15000.0);
metrics.business().update_portfolio(125000.0, 2500.0, 15.5);

// Record data quality metrics
metrics.business().record_data_processing(1000, 0.98, processing_duration);
```

#### System Metrics
```rust
// System metrics are automatically collected
// CPU, memory, disk, and network metrics are updated every 30 seconds
let summary = system_monitor.get_system_summary();
println!("CPU Usage: {:.2}%", summary.cpu_usage_percent);
println!("Memory Usage: {:.2}%", summary.memory_usage_percent);
```

#### Application Metrics
```rust
// Record HTTP request metrics
metrics.application().record_http_request("GET", "/api/v1/predict", 200, request_duration);

// Record database query metrics
metrics.application().record_database_query(query_duration, true);

// Record cache operations
metrics.application().record_cache_operation(true); // cache hit
```

### Distributed Tracing

```rust
use neural_trader::observability::tracer::{DistributedTracer, TraceMetadata};

let tracer = DistributedTracer::new(config)?;

// Start a trace
let metadata = TraceMetadata {
    user_id: Some("user123".to_string()),
    request_id: Some("req-456".to_string()),
    source_service: "neural-predictor".to_string(),
    ..Default::default()
};

let context = tracer.start_trace("model_prediction", metadata).await;

// Create child spans
let child_context = tracer.start_span(&context, "data_preprocessing").await;

// Finish traces
tracer.finish_trace(child_context, TraceStatus::Success).await?;
tracer.finish_trace(context, TraceStatus::Success).await?;
```

### Health Monitoring

```rust
use neural_trader::observability::ObservabilitySystem;

let observability = ObservabilitySystem::new(&config).await?;

// Get system health status
let health = observability.get_health_status().await;
match health.overall_status {
    HealthLevel::Healthy => println!("System is healthy"),
    HealthLevel::Warning => println!("System has warnings"),
    HealthLevel::Critical => println!("System is in critical state"),
}

// Get performance snapshot
let performance = observability.get_performance_snapshot().await;
println!("Active connections: {}", performance.active_connections);
println!("Requests per second: {:.2}", performance.requests_per_second);
```

## Production Deployment

### Docker Configuration

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --profile production

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates
COPY --from=builder /app/target/release/neural-trader /usr/local/bin/
COPY config/production.toml /etc/neural-trader/config.toml
CMD ["neural-trader"]
```

### Environment Variables for Production

```bash
# Database
DATABASE_URL=postgres://neural_trader:${DB_PASSWORD}@prod-db:5432/neural_trader_db
DATABASE_MAX_CONNECTIONS=50

# Redis
REDIS_URL=redis://:${REDIS_PASSWORD}@prod-redis:6379/0

# Security
SECURITY_ENABLE_TLS=true
SECURITY_TLS_CERT_PATH=/etc/ssl/certs/neural-trader.crt
SECURITY_TLS_KEY_PATH=/etc/ssl/private/neural-trader.key

# Monitoring
MONITORING_PROMETHEUS_PORT=8080

# Alerts
ALERTS_ENABLE_EMAIL_ALERTS=true
ALERTS_EMAIL_FROM=${ALERT_EMAIL_FROM}
ALERTS_SLACK_WEBHOOK_URL=${SLACK_WEBHOOK_URL}

# Backup
BACKUP_ENABLE_CLOUD_BACKUP=true
BACKUP_CLOUD_BACKUP_BUCKET=${S3_BACKUP_BUCKET}
BACKUP_CLOUD_BACKUP_REGION=${AWS_REGION}
```

### Prometheus Configuration

```yaml
global:
  scrape_interval: 30s

scrape_configs:
  - job_name: 'neural-trader'
    static_configs:
      - targets: ['neural-trader:8080']
    metrics_path: '/metrics'
    scrape_interval: 30s
```

### Grafana Dashboard Queries

#### System Metrics
```promql
# CPU Usage
system_cpu_usage_percent

# Memory Usage
system_memory_usage_percent

# Disk Usage
system_disk_usage_percent

# Network Traffic
rate(system_network_bytes_sent_total[5m])
rate(system_network_bytes_received_total[5m])
```

#### Business Metrics
```promql
# Prediction Rate
rate(neural_trader_predictions_total[5m])

# Prediction Accuracy
neural_trader_predictions_accuracy

# Trading Success Rate
neural_trader_trade_success_rate

# Portfolio Value
neural_trader_portfolio_value_usd
```

#### Application Metrics
```promql
# HTTP Request Rate
rate(http_requests_total[5m])

# HTTP Request Duration
histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m]))

# Database Query Duration
histogram_quantile(0.95, rate(database_query_duration_seconds_bucket[5m]))

# Error Rate
rate(errors_total[5m])
```

## Security Considerations

### TLS Configuration
- Use strong TLS certificates (minimum TLS 1.2)
- Implement certificate rotation procedures
- Monitor certificate expiration

### Rate Limiting
- Configure appropriate rate limits for your workload
- Monitor rate limit violations
- Implement dynamic rate limiting based on user behavior

### Secrets Management
- Use environment variables for sensitive configuration
- Consider using secret management systems (HashiCorp Vault, AWS Secrets Manager)
- Rotate secrets regularly

### Network Security
- Configure CORS appropriately for your domains
- Use network firewalls and security groups
- Implement request validation and sanitization

## Monitoring and Alerting

### Key Metrics to Monitor

#### System Health
- CPU usage > 80%
- Memory usage > 85%
- Disk usage > 90%
- Error rate > 5%

#### Business Metrics
- Prediction accuracy < 85%
- Trading success rate < 70%
- Model inference time > 5 seconds
- Data quality score < 95%

#### Application Performance
- HTTP request duration > 1 second
- Database query duration > 500ms
- Cache hit rate < 80%
- Active connections approaching limits

### Alert Configuration

```yaml
# Prometheus Alerting Rules
groups:
  - name: neural_trader_alerts
    rules:
      - alert: HighCPUUsage
        expr: system_cpu_usage_percent > 80
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High CPU usage detected"
          description: "CPU usage is {{ $value }}%"

      - alert: LowPredictionAccuracy
        expr: neural_trader_predictions_accuracy < 0.85
        for: 10m
        labels:
          severity: critical
        annotations:
          summary: "Prediction accuracy below threshold"
          description: "Prediction accuracy is {{ $value }}"

      - alert: HighErrorRate
        expr: rate(errors_total[5m]) > 0.05
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: "High error rate detected"
          description: "Error rate is {{ $value }} errors/second"
```

## Troubleshooting

### Common Configuration Issues

1. **Database Connection Failures**
   - Check `DATABASE_URL` format
   - Verify network connectivity
   - Check connection pool settings

2. **Redis Connection Issues**
   - Verify Redis server availability
   - Check authentication settings
   - Monitor connection pool usage

3. **TLS Certificate Problems**
   - Verify certificate file paths
   - Check certificate validity
   - Ensure proper file permissions

4. **High Memory Usage**
   - Check neural network memory allocation
   - Monitor cache sizes
   - Review connection pool settings

### Debugging Commands

```bash
# Check configuration loading
cargo run -- --validate-config

# Test database connectivity
cargo run -- --test-db

# Verify metrics endpoint
curl http://localhost:8080/metrics

# Check system health
cargo run -- --health-check
```

## Performance Optimization

### Database Optimization
- Use connection pooling with appropriate limits
- Implement query timeouts
- Monitor slow queries
- Consider read replicas for read-heavy workloads

### Redis Optimization
- Configure appropriate TTL values
- Monitor memory usage
- Use Redis clustering for high availability
- Implement proper cache invalidation strategies

### Application Performance
- Use async/await for I/O operations
- Configure appropriate worker thread counts
- Implement request timeouts
- Use circuit breakers for external dependencies

## Conclusion

The production configuration and observability system provides a robust foundation for deploying the Neural Trader Autonomous Platform in production environments. The system includes comprehensive monitoring, security features, and operational tools necessary for running a reliable trading platform.

Key benefits:
- **Comprehensive Configuration Management**: Environment-specific configs with validation
- **Real-time Monitoring**: Business, system, and application metrics
- **Operational Excellence**: Structured logging, tracing, and alerting
- **Security**: TLS, rate limiting, and sensitive data protection
- **Reliability**: Circuit breakers, graceful shutdown, and backup management

For additional support or questions, refer to the implementation details in the source code or create an issue in the project repository.