# Configuration Guide

## Overview

This guide covers all configuration options for the data backfill system, including environment variables, configuration files, and runtime parameters.

## Configuration Hierarchy

Configuration is loaded in the following order (later sources override earlier ones):

1. Default values (built-in)
2. Configuration files
3. Environment variables
4. Command-line arguments

## Configuration Files

### Main Configuration File

Location: `~/.neural_trader/backfill.yaml` or `/etc/neural_trader/backfill.yaml`

```yaml
# Complete configuration example
backfill:
  # General settings
  defaults:
    batch_size: 10000
    workers: 10
    checkpoint_enabled: true
    memory_limit_mb: 2048
    log_level: INFO
    
  # Performance tuning
  performance:
    download_chunk_size: 1048576  # 1MB
    db_pool_size: 20
    db_batch_size: 5000
    max_retries: 3
    retry_backoff: 2.0
    
  # File handling
  files:
    supported_formats:
      - csv
      - json
      - parquet
    compression:
      - gzip
      - bz2
      - xz
    max_file_size_gb: 10
    
# S3 Configuration
s3:
  profile: polygon-s3
  region: us-east-1
  bucket: flatfiles
  endpoint_url: null  # Custom endpoint for S3-compatible storage
  
  # Download settings
  download:
    max_concurrent: 10
    chunk_size: 8388608  # 8MB
    multipart_threshold: 104857600  # 100MB
    max_bandwidth_mbps: 0  # 0 = unlimited
    
  # Local storage
  storage:
    base_path: /mnt/external/polygon_data
    temp_path: /tmp/neural_trader/downloads
    organize_by_date: true
    
# Database Configuration
database:
  # Connection
  host: localhost
  port: 5432
  name: trading
  schema: public
  
  # Credentials (use env vars in production)
  username: ${DB_USER}
  password: ${DB_PASSWORD}
  
  # Connection pool
  pool:
    min_size: 10
    max_size: 20
    acquire_timeout: 30
    idle_timeout: 600
    
  # TimescaleDB specific
  timescale:
    compression_enabled: true
    chunk_time_interval: 1 day
    retention_policy: 2555 days  # 7 years
    
# Redis Configuration (for checkpoints)
redis:
  host: localhost
  port: 6379
  db: 0
  password: ${REDIS_PASSWORD}
  
  # Connection settings
  socket_timeout: 30
  socket_connect_timeout: 10
  retry_on_timeout: true
  
  # Pool settings
  pool:
    max_connections: 50
    
# Monitoring Configuration
monitoring:
  # Metrics
  metrics:
    enabled: true
    port: 8000
    path: /metrics
    
  # Health checks
  health:
    enabled: true
    port: 8080
    path: /health
    
  # Logging
  logging:
    level: INFO
    format: json
    
    # File logging
    file:
      enabled: true
      path: /var/log/neural_trader/backfill.log
      max_size_mb: 100
      backup_count: 10
      
    # Syslog
    syslog:
      enabled: false
      host: localhost
      port: 514
      facility: local0
      
# Validation Configuration
validation:
  # Data quality checks
  quality:
    check_ohlc_consistency: true
    check_timestamps: true
    check_duplicates: true
    max_price_change_percent: 50
    
  # Thresholds
  thresholds:
    max_bad_records_percent: 1.0
    min_records_per_file: 100
    
# Security Configuration
security:
  # API authentication
  api:
    enabled: true
    key_header: X-API-Key
    keys_file: /etc/neural_trader/api_keys.json
    
  # Encryption
  encryption:
    at_rest: true
    algorithm: AES256
    key_file: ${ENCRYPTION_KEY_FILE}
    
  # Network
  network:
    ssl_verify: true
    min_tls_version: "1.2"
```

### Symbol Lists

Location: `~/.neural_trader/symbols/`

```yaml
# symbols/sp500.yaml
symbols:
  - AAPL
  - MSFT
  - GOOGL
  - AMZN
  # ... more symbols

# symbols/watchlist.yaml
symbols:
  - TSLA
  - NVDA
  - AMD
```

## Environment Variables

### Core Settings

```bash
# Application settings
export NEURAL_TRADER_ENV=production
export NEURAL_TRADER_CONFIG=/path/to/config.yaml
export NEURAL_TRADER_LOG_LEVEL=INFO

# Performance
export BACKFILL_WORKERS=10
export BACKFILL_BATCH_SIZE=10000
export BACKFILL_MEMORY_LIMIT=2048  # MB
export BACKFILL_MAX_RETRIES=3

# Timeouts (seconds)
export BACKFILL_DOWNLOAD_TIMEOUT=300
export BACKFILL_DB_TIMEOUT=60
export BACKFILL_REDIS_TIMEOUT=30
```

### AWS Configuration

```bash
# AWS credentials (prefer AWS profiles)
export AWS_PROFILE=polygon-s3
export AWS_DEFAULT_REGION=us-east-1
export AWS_ACCESS_KEY_ID=your-key-id
export AWS_SECRET_ACCESS_KEY=your-secret-key

# S3 specific
export S3_BUCKET=flatfiles
export S3_ENDPOINT_URL=https://s3.amazonaws.com
export S3_MAX_CONCURRENT_REQUESTS=10
export S3_MAX_BANDWIDTH=104857600  # bytes/sec
```

### Database Configuration

```bash
# PostgreSQL/TimescaleDB
export DB_HOST=localhost
export DB_PORT=5432
export DB_NAME=trading
export DB_USER=backfill_user
export DB_PASSWORD=secure_password
export DB_SCHEMA=public

# Connection pool
export DB_POOL_MIN_SIZE=10
export DB_POOL_MAX_SIZE=20
export DB_POOL_TIMEOUT=30

# TimescaleDB
export TIMESCALE_COMPRESSION=on
export TIMESCALE_CHUNK_INTERVAL=86400  # seconds
```

### Redis Configuration

```bash
# Redis connection
export REDIS_HOST=localhost
export REDIS_PORT=6379
export REDIS_DB=0
export REDIS_PASSWORD=redis_password

# Redis settings
export REDIS_MAX_CONNECTIONS=50
export REDIS_SOCKET_TIMEOUT=30
```

### Monitoring Configuration

```bash
# Metrics
export METRICS_ENABLED=true
export METRICS_PORT=8000
export METRICS_PATH=/metrics

# Logging
export LOG_FORMAT=json
export LOG_FILE=/var/log/neural_trader/backfill.log
export LOG_MAX_SIZE=104857600  # 100MB
export LOG_BACKUP_COUNT=10

# Alerts
export ALERT_WEBHOOK_URL=https://hooks.slack.com/services/xxx
export ALERT_EMAIL=ops@company.com
```

## Command-Line Arguments

Command-line arguments override all other configuration sources:

```bash
# Override configuration file
python -m data_ingestion.backfill \
  --config /custom/config.yaml \
  --workers 20 \
  --batch-size 50000 \
  --log-level DEBUG

# Override S3 settings
python -m data_ingestion.backfill s3 \
  --profile custom-profile \
  --bucket different-bucket \
  --region eu-west-1 \
  --max-bandwidth 52428800  # 50MB/s

# Override database settings
python -m data_ingestion.backfill \
  --db-host remote-host.com \
  --db-port 5433 \
  --db-pool-size 30
```

## Configuration Profiles

Use profiles for different environments:

### Development Profile

```yaml
# config/development.yaml
backfill:
  defaults:
    workers: 2
    batch_size: 1000
    log_level: DEBUG
    
database:
  host: localhost
  name: trading_dev
  
redis:
  host: localhost
  db: 1
  
monitoring:
  metrics:
    enabled: false
```

### Production Profile

```yaml
# config/production.yaml
backfill:
  defaults:
    workers: 20
    batch_size: 50000
    log_level: INFO
    
database:
  host: db-prod.company.com
  name: trading
  pool:
    max_size: 50
    
redis:
  host: redis-prod.company.com
  password: ${REDIS_PASSWORD}
  
monitoring:
  metrics:
    enabled: true
  alerts:
    enabled: true
```

### Load Profile

```bash
# Load specific profile
export NEURAL_TRADER_ENV=production
python -m data_ingestion.backfill --config config/${NEURAL_TRADER_ENV}.yaml
```

## Performance Tuning

### Memory Configuration

```yaml
performance:
  # Memory allocation
  memory:
    heap_size_mb: 4096
    buffer_size_mb: 512
    cache_size_mb: 1024
    
  # Garbage collection
  gc:
    threshold0: 700
    threshold1: 10
    threshold2: 10
```

### I/O Configuration

```yaml
performance:
  io:
    # File reading
    read_buffer_size: 65536
    prefetch_size: 1048576
    
    # Network
    tcp_nodelay: true
    tcp_keepalive: true
    socket_buffer_size: 262144
```

### Database Optimization

```yaml
database:
  optimization:
    # Batch operations
    batch_insert: true
    use_copy_from: true
    
    # Query optimization
    prepared_statements: true
    statement_cache_size: 1000
    
    # Connection tuning
    tcp_keepalives_idle: 60
    tcp_keepalives_interval: 10
    tcp_keepalives_count: 6
```

## Validation Configuration

### Data Quality Rules

```yaml
validation:
  rules:
    # OHLC validation
    ohlc:
      enabled: true
      strict: false
      tolerance_percent: 0.001
      
    # Timestamp validation
    timestamps:
      enabled: true
      timezone: UTC
      format: ISO8601
      
    # Symbol validation
    symbols:
      enabled: true
      pattern: "^[A-Z0-9\\-\\.]{1,10}$"
      
    # Volume validation
    volume:
      enabled: true
      min_value: 0
      max_value: 1000000000
```

### Custom Validators

```yaml
validation:
  custom:
    - name: price_spike_detector
      module: validators.price_spike
      config:
        max_change_percent: 20
        
    - name: gap_detector
      module: validators.gaps
      config:
        max_gap_minutes: 5
```

## Security Configuration

### Authentication

```yaml
security:
  auth:
    # API keys
    api_keys:
      enabled: true
      rotation_days: 90
      
    # OAuth2 (optional)
    oauth2:
      enabled: false
      provider: auth0
      domain: company.auth0.com
      
    # mTLS (optional)
    mtls:
      enabled: false
      ca_cert: /etc/ssl/ca.crt
      require_client_cert: true
```

### Encryption

```yaml
security:
  encryption:
    # Data at rest
    at_rest:
      enabled: true
      algorithm: AES256-GCM
      key_rotation_days: 30
      
    # Data in transit
    in_transit:
      tls_version: "1.3"
      cipher_suites:
        - TLS_AES_256_GCM_SHA384
        - TLS_CHACHA20_POLY1305_SHA256
```

## Monitoring and Alerts

### Alert Rules

```yaml
monitoring:
  alerts:
    rules:
      - name: high_error_rate
        condition: error_rate > 0.05
        severity: critical
        notify:
          - email
          - slack
          
      - name: slow_processing
        condition: processing_rate < 5000
        severity: warning
        notify:
          - slack
          
      - name: disk_space_low
        condition: disk_free_percent < 10
        severity: critical
        notify:
          - email
          - pagerduty
```

### Notification Channels

```yaml
monitoring:
  notifications:
    email:
      smtp_host: smtp.gmail.com
      smtp_port: 587
      from: alerts@company.com
      to:
        - ops@company.com
        - oncall@company.com
        
    slack:
      webhook_url: ${SLACK_WEBHOOK_URL}
      channel: "#ops-alerts"
      
    pagerduty:
      api_key: ${PAGERDUTY_API_KEY}
      service_id: PXXXXXX
```

## Troubleshooting Configuration

### Debug Settings

```yaml
debug:
  # Verbose logging
  verbose: true
  log_sql: true
  log_requests: true
  
  # Profiling
  profiling:
    enabled: true
    output: /tmp/neural_trader/profiles/
    
  # Tracing
  tracing:
    enabled: true
    sample_rate: 0.1
    exporter: jaeger
    endpoint: http://localhost:14268
```

### Common Issues

1. **Configuration not loading**
   ```bash
   # Check configuration file location
   python -m data_ingestion.backfill --show-config
   
   # Validate configuration
   python -m data_ingestion.backfill --validate-config
   ```

2. **Environment variables not working**
   ```bash
   # Debug environment
   python -m data_ingestion.backfill --debug-env
   ```

3. **Permission errors**
   ```bash
   # Check file permissions
   ls -la ~/.neural_trader/
   chmod 600 ~/.neural_trader/backfill.yaml
   ```

## Best Practices

1. **Use environment variables for secrets**
   - Never commit passwords or API keys
   - Use `${VAR_NAME}` syntax in config files

2. **Profile-based configuration**
   - Separate configs for dev/staging/prod
   - Use NEURAL_TRADER_ENV variable

3. **Monitor configuration changes**
   - Version control config files
   - Log configuration on startup
   - Alert on unexpected changes

4. **Regular validation**
   - Validate config before deployment
   - Test with dry-run mode
   - Monitor for deprecated options