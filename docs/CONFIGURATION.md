# Neural Trader Platform Configuration Reference

## Overview

The Neural Trader Autonomous Platform uses a hierarchical configuration system that combines TOML files, environment variables, and runtime parameters. This document provides comprehensive reference for all configuration options.

## Configuration Hierarchy

Configuration values are applied in the following order (later values override earlier ones):

1. **Default Values** - Compiled-in defaults
2. **Configuration Files** - TOML files in `config/` directory
3. **Environment Variables** - Runtime environment overrides
4. **Command Line Arguments** - Direct runtime parameters (planned)

## Configuration File Structure

### Main Configuration File: `config/platform.toml`

```toml
[platform]
name = "neural-trader-autonomous"
version = "0.1.0"

[database]
url = "postgres://neural_trader:neural_trader_pass@localhost/neural_trader_db"
max_connections = 20
min_connections = 5

[redis]
url = "redis://localhost:6379"
max_connections = 10
default_ttl_seconds = 3600

[neural]
memory_gb = 1.0
models = ["NHITS", "DeepAR", "TCN", "MLP"]
prediction_cache_ttl = 300

[monitoring]
metrics_interval_secs = 60
quality_threshold = 0.95
```

## Configuration Sections

### Platform Section

Basic platform metadata and identification.

```toml
[platform]
name = "neural-trader-autonomous"     # Platform instance name
version = "0.1.0"                     # Configuration version
```

**Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `name` | String | `"neural-trader-autonomous"` | Platform instance identifier |
| `version` | String | `"0.1.0"` | Configuration schema version |

### Database Section

PostgreSQL/TimescaleDB configuration for time series storage.

```toml
[database]
# Connection string with credentials
url = "postgres://username:password@hostname:port/database_name"

# Connection pool settings
max_connections = 20        # Maximum concurrent connections
min_connections = 5         # Minimum connections to maintain

# Optional: Connection timeout settings
connection_timeout_secs = 30
idle_timeout_secs = 600
max_lifetime_secs = 1800
```

**Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `url` | String | Required | PostgreSQL connection string |
| `max_connections` | Integer | `20` | Maximum database connections |
| `min_connections` | Integer | `5` | Minimum database connections |
| `connection_timeout_secs` | Integer | `30` | Connection establishment timeout |
| `idle_timeout_secs` | Integer | `600` | Idle connection timeout |
| `max_lifetime_secs` | Integer | `1800` | Maximum connection lifetime |

**Environment Variable Overrides:**

```bash
export DATABASE_URL="postgres://user:pass@host:5432/db"
export DATABASE_MAX_CONNECTIONS=50
export DATABASE_MIN_CONNECTIONS=10
export DATABASE_CONNECTION_TIMEOUT_SECS=60
export DATABASE_IDLE_TIMEOUT_SECS=300
export DATABASE_MAX_LIFETIME_SECS=3600
```

### Redis Section

Redis configuration for caching and real-time data.

```toml
[redis]
# Connection URL
url = "redis://hostname:port"
# For authenticated Redis: "redis://:password@hostname:port"
# For Redis Cluster: "redis://host1:port1,host2:port2,host3:port3"

# Connection pool settings
max_connections = 10        # Maximum concurrent connections
min_connections = 2         # Minimum connections to maintain

# Cache behavior
default_ttl_seconds = 3600  # Default time-to-live for cached items
max_key_size = 512          # Maximum key length in bytes
max_value_size = 1048576    # Maximum value size in bytes (1MB)

# Optional: Advanced Redis settings
connection_timeout_ms = 5000    # Connection timeout in milliseconds
response_timeout_ms = 3000      # Response timeout in milliseconds
reconnect_delay_ms = 1000       # Delay between reconnection attempts
max_retry_attempts = 3          # Maximum retry attempts for failed operations
```

**Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `url` | String | Required | Redis connection URL |
| `max_connections` | Integer | `10` | Maximum Redis connections |
| `min_connections` | Integer | `2` | Minimum Redis connections |
| `default_ttl_seconds` | Integer | `3600` | Default cache TTL (1 hour) |
| `max_key_size` | Integer | `512` | Maximum key size in bytes |
| `max_value_size` | Integer | `1048576` | Maximum value size (1MB) |
| `connection_timeout_ms` | Integer | `5000` | Connection timeout |
| `response_timeout_ms` | Integer | `3000` | Response timeout |
| `reconnect_delay_ms` | Integer | `1000` | Reconnection delay |
| `max_retry_attempts` | Integer | `3` | Maximum retry attempts |

**Environment Variable Overrides:**

```bash
export REDIS_URL="redis://localhost:6379"
export REDIS_MAX_CONNECTIONS=20
export REDIS_MIN_CONNECTIONS=5
export REDIS_DEFAULT_TTL_SECONDS=7200
export REDIS_CONNECTION_TIMEOUT_MS=10000
export REDIS_RESPONSE_TIMEOUT_MS=5000
```

### Neural Network Section

Configuration for ML models and neural network operations.

```toml
[neural]
# Memory allocation for neural operations
memory_gb = 2.0             # Memory allocation in GB

# Available neural network models
models = [
    "NHITS",               # Neural Hierarchical Interpolation for Time Series
    "DeepAR",              # Deep Autoregressive model
    "TCN",                 # Temporal Convolutional Network
    "MLP",                 # Multi-Layer Perceptron
    "LSTM",                # Long Short-Term Memory
    "GRU",                 # Gated Recurrent Unit
    "Transformer"          # Transformer architecture
]

# Prediction caching
prediction_cache_ttl = 300  # Cache TTL for predictions (5 minutes)
max_batch_size = 1000       # Maximum batch size for predictions

# Model performance settings
thread_pool_size = 4        # Number of threads for model inference
gpu_memory_fraction = 0.8   # Fraction of GPU memory to use (if available)
model_timeout_secs = 30     # Timeout for model operations

# Training configuration
training_data_limit = 100000    # Maximum training samples per model
validation_split = 0.2          # Fraction of data for validation
max_epochs = 100                # Maximum training epochs
early_stopping_patience = 10    # Early stopping patience
```

**Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `memory_gb` | Float | `1.0` | Memory allocation for neural operations |
| `models` | Array[String] | `["NHITS", "DeepAR", "TCN", "MLP"]` | Enabled neural models |
| `prediction_cache_ttl` | Integer | `300` | Prediction cache TTL (seconds) |
| `max_batch_size` | Integer | `1000` | Maximum prediction batch size |
| `thread_pool_size` | Integer | `4` | Inference thread pool size |
| `gpu_memory_fraction` | Float | `0.8` | GPU memory usage fraction |
| `model_timeout_secs` | Integer | `30` | Model operation timeout |
| `training_data_limit` | Integer | `100000` | Max training samples |
| `validation_split` | Float | `0.2` | Validation data fraction |
| `max_epochs` | Integer | `100` | Maximum training epochs |
| `early_stopping_patience` | Integer | `10` | Early stopping patience |

**Environment Variable Overrides:**

```bash
export NEURAL_MEMORY_GB=4.0
export NEURAL_MODELS="NHITS,DeepAR,LSTM,Transformer"
export NEURAL_PREDICTION_CACHE_TTL=600
export NEURAL_MAX_BATCH_SIZE=2000
export NEURAL_THREAD_POOL_SIZE=8
export NEURAL_GPU_MEMORY_FRACTION=0.9
```

### Monitoring Section

Configuration for metrics collection, alerting, and system monitoring.

```toml
[monitoring]
# Metrics collection interval
metrics_interval_secs = 60      # How often to collect metrics

# Quality thresholds
quality_threshold = 0.95        # Minimum acceptable data quality (95%)
latency_threshold_ms = 100      # Maximum acceptable latency
error_rate_threshold = 0.05     # Maximum acceptable error rate (5%)

# Alerting configuration
enable_alerts = true            # Enable/disable alerting system
alert_channels = ["email", "slack"]  # Alert notification channels
max_alerts_per_hour = 10        # Rate limiting for alerts

# Metrics storage
metrics_retention_days = 30     # How long to keep metrics data
metrics_aggregation_interval = 300  # Metrics aggregation interval (5min)

# Health check configuration
health_check_interval_secs = 30     # Health check frequency
health_check_timeout_secs = 5       # Health check timeout
max_consecutive_failures = 3        # Max failures before marking unhealthy

# Performance monitoring
enable_profiling = false        # Enable performance profiling
profiling_sample_rate = 0.01    # Profiling sample rate (1%)
memory_usage_threshold = 0.85   # Memory usage alert threshold (85%)
cpu_usage_threshold = 0.8       # CPU usage alert threshold (80%)
disk_usage_threshold = 0.9      # Disk usage alert threshold (90%)
```

**Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `metrics_interval_secs` | Integer | `60` | Metrics collection interval |
| `quality_threshold` | Float | `0.95` | Minimum data quality threshold |
| `latency_threshold_ms` | Integer | `100` | Maximum latency threshold |
| `error_rate_threshold` | Float | `0.05` | Maximum error rate threshold |
| `enable_alerts` | Boolean | `true` | Enable alerting system |
| `alert_channels` | Array[String] | `["email"]` | Alert notification channels |
| `max_alerts_per_hour` | Integer | `10` | Alert rate limiting |
| `metrics_retention_days` | Integer | `30` | Metrics retention period |
| `health_check_interval_secs` | Integer | `30` | Health check frequency |
| `enable_profiling` | Boolean | `false` | Enable performance profiling |
| `memory_usage_threshold` | Float | `0.85` | Memory usage alert threshold |

**Environment Variable Overrides:**

```bash
export MONITORING_METRICS_INTERVAL_SECS=30
export MONITORING_QUALITY_THRESHOLD=0.98
export MONITORING_LATENCY_THRESHOLD_MS=50
export MONITORING_ENABLE_ALERTS=true
export MONITORING_MAX_ALERTS_PER_HOUR=20
```

## Advanced Configuration

### Logging Configuration

```toml
[logging]
level = "info"                  # Log level: trace, debug, info, warn, error
format = "json"                 # Log format: json, plain
output = "file"                 # Output: stdout, stderr, file
file_path = "/var/log/neural-trader/app.log"
max_file_size_mb = 100          # Maximum log file size
max_files = 10                  # Maximum number of log files to keep
compress_old_files = true       # Compress rotated log files

# Module-specific log levels
[logging.modules]
database = "debug"              # Database operations
neural = "info"                 # Neural network operations
trading = "warn"                # Trading operations
network = "error"               # Network communications
```

### Security Configuration

```toml
[security]
# API security
enable_tls = true               # Enable TLS/SSL
tls_cert_path = "/etc/ssl/certs/neural-trader.crt"
tls_key_path = "/etc/ssl/private/neural-trader.key"
min_tls_version = "1.2"         # Minimum TLS version

# Authentication
enable_auth = true              # Enable authentication
auth_method = "jwt"             # Authentication method: jwt, oauth2, api_key
jwt_secret = "your-secret-key" # JWT signing secret (use env var)
jwt_expiry_hours = 24          # JWT token expiry

# Rate limiting
enable_rate_limiting = true     # Enable API rate limiting
rate_limit_requests_per_minute = 100
rate_limit_burst = 20

# CORS configuration
enable_cors = true              # Enable CORS
cors_allowed_origins = ["https://trading-ui.example.com"]
cors_allowed_methods = ["GET", "POST", "PUT", "DELETE"]
cors_allowed_headers = ["Content-Type", "Authorization"]
```

### Trading Configuration

```toml
[trading]
# Risk management
max_position_size = 0.1         # Maximum position size (10% of portfolio)
max_daily_loss = 0.05           # Maximum daily loss (5%)
stop_loss_percentage = 0.02     # Default stop loss (2%)
take_profit_percentage = 0.04   # Default take profit (4%)

# Order execution
default_order_type = "market"   # Default order type: market, limit
order_timeout_secs = 30         # Order execution timeout
max_slippage_percentage = 0.005 # Maximum acceptable slippage (0.5%)

# Portfolio management
initial_balance = 10000.0       # Initial portfolio balance
min_trade_amount = 10.0         # Minimum trade amount
max_open_positions = 5          # Maximum concurrent positions
rebalance_frequency_hours = 24  # Portfolio rebalancing frequency
```

## Environment-Specific Configuration

### Development Configuration

```toml
# config/development.toml
[platform]
name = "neural-trader-dev"

[database]
url = "postgres://dev_user:dev_pass@localhost/neural_trader_dev"
max_connections = 5

[neural]
memory_gb = 0.5
models = ["MLP"]  # Use simpler models for development

[monitoring]
metrics_interval_secs = 10
quality_threshold = 0.8

[logging]
level = "debug"
output = "stdout"
```

### Production Configuration

```toml
# config/production.toml
[platform]
name = "neural-trader-prod"

[database]
url = "postgres://prod_user:${DATABASE_PASSWORD}@db-cluster:5432/neural_trader"
max_connections = 50
min_connections = 10

[neural]
memory_gb = 8.0
models = ["NHITS", "DeepAR", "TCN", "LSTM", "Transformer"]
thread_pool_size = 16

[monitoring]
metrics_interval_secs = 30
quality_threshold = 0.98
enable_alerts = true

[security]
enable_tls = true
enable_auth = true
enable_rate_limiting = true

[logging]
level = "info"
format = "json"
output = "file"
```

## Configuration Validation

The platform performs comprehensive validation of configuration values:

### Validation Rules

1. **Database Configuration:**
   - URL must be a valid PostgreSQL connection string
   - `max_connections` must be greater than `min_connections`
   - Connection counts must be positive integers

2. **Redis Configuration:**
   - URL must be a valid Redis connection string
   - TTL values must be positive integers
   - Connection counts must be positive integers

3. **Neural Configuration:**
   - Memory allocation must be positive
   - At least one model must be specified
   - Thread pool size must be positive
   - Training parameters must be within valid ranges

4. **Monitoring Configuration:**
   - Quality threshold must be between 0.0 and 1.0
   - Interval values must be positive integers
   - Thresholds must be within reasonable ranges

### Validation Examples

```rust
// Configuration validation in code
use autonomous_platform::{PlatformConfig, load_default_config};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load and validate configuration
    let config = load_default_config()?;
    
    // Configuration is automatically validated during loading
    println!("Configuration loaded successfully: {}", config.platform.name);
    
    Ok(())
}
```

## Configuration Best Practices

### Security Best Practices

1. **Never commit secrets to version control:**
   ```bash
   # Use environment variables for sensitive data
   export DATABASE_PASSWORD="$(cat /run/secrets/db_password)"
   export JWT_SECRET="$(openssl rand -base64 32)"
   ```

2. **Use different configurations per environment:**
   ```bash
   # Load environment-specific configuration
   export CONFIG_PATH="config/production.toml"
   ```

3. **Validate configuration in CI/CD:**
   ```bash
   # Add configuration validation to your CI pipeline
   cargo run --bin validate-config config/production.toml
   ```

### Performance Best Practices

1. **Tune connection pools based on workload:**
   ```toml
   [database]
   # For high-throughput workloads
   max_connections = 50
   min_connections = 10
   ```

2. **Optimize cache settings:**
   ```toml
   [redis]
   # Balance memory usage vs. performance
   default_ttl_seconds = 1800  # 30 minutes
   max_connections = 20        # Based on expected load
   ```

3. **Configure neural networks for your hardware:**
   ```toml
   [neural]
   memory_gb = 8.0            # Based on available RAM
   thread_pool_size = 16      # Based on CPU cores
   gpu_memory_fraction = 0.8  # If GPU available
   ```

### Monitoring Best Practices

1. **Set appropriate thresholds:**
   ```toml
   [monitoring]
   quality_threshold = 0.95    # 95% data quality
   latency_threshold_ms = 100  # 100ms max latency
   error_rate_threshold = 0.01 # 1% max error rate
   ```

2. **Configure alerting channels:**
   ```toml
   [monitoring]
   alert_channels = ["email", "slack", "pagerduty"]
   max_alerts_per_hour = 5     # Prevent alert spam
   ```

## Configuration Examples

### Minimal Configuration

```toml
# Minimal working configuration
[platform]
name = "neural-trader"

[database]
url = "postgres://user:pass@localhost/neural_trader"

[redis]
url = "redis://localhost"

[neural]
models = ["MLP"]
```

### High-Performance Configuration

```toml
# Optimized for high-performance trading
[platform]
name = "neural-trader-hft"

[database]
url = "postgres://user:pass@db-cluster/neural_trader"
max_connections = 100
connection_timeout_secs = 5

[redis]
url = "redis://redis-cluster:6379"
max_connections = 50
default_ttl_seconds = 300
response_timeout_ms = 1000

[neural]
memory_gb = 16.0
models = ["NHITS", "DeepAR", "Transformer"]
thread_pool_size = 32
max_batch_size = 5000
gpu_memory_fraction = 0.95

[monitoring]
metrics_interval_secs = 5
quality_threshold = 0.99
latency_threshold_ms = 10

[trading]
max_position_size = 0.05
order_timeout_secs = 5
max_slippage_percentage = 0.001
```

### Multi-Environment Configuration

```bash
# Use different configurations per environment
# Development
export CONFIG_PATH="config/dev.toml"
export RUST_LOG="debug"

# Staging
export CONFIG_PATH="config/staging.toml"
export RUST_LOG="info"

# Production
export CONFIG_PATH="config/prod.toml"
export RUST_LOG="warn"
export DATABASE_PASSWORD="$(cat /run/secrets/db_password)"
export REDIS_AUTH_TOKEN="$(cat /run/secrets/redis_token)"
```

This configuration reference provides comprehensive documentation for all platform settings. Always validate your configuration in a staging environment before deploying to production.