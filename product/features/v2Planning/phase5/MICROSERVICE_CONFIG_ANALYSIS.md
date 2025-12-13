# Microservice Environment Configuration Analysis Report

## Executive Summary

After deep analysis of all microservices and the neural-core library, I've identified the exact environment configuration requirements for each service. The analysis reveals that **neural-core** provides shared EventBus configuration that must be properly layered into both **neural-ml-ops** and **neural-trading** services.

## Service Configuration Requirements

### 1. Config-Store Service

**Purpose**: Central configuration management and distribution

**Required Environment Variables**:
```yaml
# Service Identity
SERVICE_NAME: config-store
CONFIG_ENV: ${CONFIG_ENV:-dev}

# Git Repository Configuration
CONFIG_REPO_URL: ${CONFIG_REPO_URL}  # Git repo for config files
CONFIG_BRANCH: ${CONFIG_BRANCH:-main}
CONFIG_PATH: /configs  # Local path for configs

# Redis Configuration
REDIS_URL: redis://redis:6379
REDIS_DB: 0
REDIS_KEY_PREFIX: "config:"
REDIS_CACHE_TTL: 3600

# gRPC Server
GRPC_PORT: 50051
GRPC_MAX_CONNECTIONS: 100

# Health Check
HEALTH_PORT: 8090
HEALTH_PATH: /health

# Logging
LOG_LEVEL: ${LOG_LEVEL:-info}
LOG_FORMAT: json

# Monitoring
METRICS_ENABLED: true
METRICS_PORT: 9091
```

### 2. Data-Ingestion Service

**Purpose**: Ingest market data from various sources

**Required Environment Variables**:
```yaml
# Service Identity
SERVICE_NAME: data-ingestion
CONFIG_ENV: ${CONFIG_ENV:-dev}

# Config Store Connection
CONFIG_STORE_URL: config-store:50051

# Redis Configuration (for raw data stream)
REDIS_URL: redis://redis:6379
REDIS_STREAM: market_data_raw
REDIS_MAX_LEN: 10000
REDIS_CONSUMER_GROUP: data-ingestion

# API Server
API_PORT: 8081
API_HOST: 0.0.0.0
API_RATE_LIMIT: 1000
API_TIMEOUT: 30
API_MAX_CONNECTIONS: 100

# Data Sources
POLYGON_API_KEY: ${POLYGON_API_KEY}
POLYGON_RATE_LIMIT: 5
ALPHA_VANTAGE_API_KEY: ${ALPHA_VANTAGE_API_KEY}
ALPHA_VANTAGE_RATE_LIMIT: 5
SYNTHETIC_DATA_ENABLED: true
SYNTHETIC_DATA_INTERVAL: 5

# Buffer Configuration
BUFFER_SIZE: 10000
FLUSH_INTERVAL_MS: 1000

# Retry Configuration
RETRY_MAX_ATTEMPTS: 3
RETRY_BACKOFF_MS: 1000
RETRY_MAX_BACKOFF_MS: 30000

# Logging & Monitoring
LOG_LEVEL: ${LOG_LEVEL:-info}
METRICS_PORT: 9091
HEALTH_PORT: 8081
```

### 3. Data-Staging Service

**Purpose**: Transform JSON to Proto and validate data quality

**Required Environment Variables**:
```yaml
# Service Identity
SERVICE_NAME: data-staging
CONFIG_ENV: ${CONFIG_ENV:-dev}
SOURCE_ID: data-staging
DOMAIN: trading

# Config Store Connection
CONFIG_STORE_URL: config-store:50051

# Redis Configuration (consumer)
REDIS_URL: redis://redis:6379
REDIS_INPUT_STREAM: market_data_raw
REDIS_OUTPUT_STREAM: market_data_staged
REDIS_CONSUMER_GROUP: data-staging
REDIS_BATCH_SIZE: 100
REDIS_BLOCK_TIMEOUT_MS: 5000

# TimescaleDB Configuration
TIMESCALE_URL: postgresql://postgres:${POSTGRES_PASSWORD:-postgres}@timescaledb:5432/neural_trader
TIMESCALE_POOL_SIZE: 10
TIMESCALE_TIMEOUT_SECONDS: 30

# Data Processing
TRANSFORM_VERBOSE: false
QUALITY_THRESHOLD: 0.7
DLQ_ENABLED: true
DLQ_MAX_RETRIES: 3

# EventBus Configuration (inherited from neural-core)
EVENTBUS_TYPE: redis
EVENTBUS_URL: ${REDIS_URL}
EVENTBUS_CHANNELS: ["market_data.*", "staging.*"]

# Logging & Monitoring
LOG_LEVEL: ${LOG_LEVEL:-info}
METRICS_PORT: 9092
HEALTH_PORT: 50052
```

### 4. Neural-Core Library Configuration

**Purpose**: Shared library providing EventBus and core types

**Configuration Requirements** (inherited by neural-ml-ops and neural-trading):
```yaml
# EventBus Configuration
EVENTBUS_TYPE: redis  # Options: redis, inmemory
EVENTBUS_URL: redis://redis:6379
EVENTBUS_DEFAULT_GROUP: neural-trader
EVENTBUS_DEFAULT_CONSUMER: ${SERVICE_NAME}-${HOSTNAME}
EVENTBUS_BATCH_SIZE: 10
EVENTBUS_BLOCK_TIMEOUT_MS: 5000
EVENTBUS_ACK_TIMEOUT_MS: 30000
EVENTBUS_BUFFER_SIZE: 1024
EVENTBUS_RECEIVE_TIMEOUT_MS: 30000

# Channel Configuration
EVENTBUS_CHANNELS: []  # Service-specific channels

# Backpressure Control
BACKPRESSURE_ENABLED: true
BACKPRESSURE_HIGH_WATERMARK: 1000
BACKPRESSURE_LOW_WATERMARK: 100

# DLQ Configuration
DLQ_ENABLED: true
DLQ_MAX_RETRIES: 3
DLQ_RETRY_DELAY_MS: 1000

# Batching Configuration
BATCH_ENABLED: true
BATCH_SIZE: 100
BATCH_TIMEOUT_MS: 1000
```

### 5. Neural-ML-Ops Service

**Purpose**: ML training coordination and feature engineering

**Required Environment Variables**:
```yaml
# Service Identity
SERVICE_NAME: neural-ml-ops
CONFIG_ENV: ${CONFIG_ENV:-dev}

# Config Store Connection
CONFIG_STORE_URL: config-store:50051

# INHERIT ALL NEURAL-CORE EVENTBUS CONFIG
# Plus override these specific channels:
EVENTBUS_CHANNELS: ["ml:features:*", "ml:models:*", "ml:training:*"]

# Model Storage
MODEL_PATH: /app/models
MODEL_REGISTRY_TYPE: filesystem  # Options: filesystem, s3
MODEL_RETENTION_DAYS: 90

# Feature Store
FEATURE_STORE_TYPE: memory  # Options: memory, redis, postgres
FEATURE_CACHE_SIZE: 10000
FEATURE_TTL_SECONDS: 3600

# Training Configuration
TRAINING_BATCH_SIZE: 32
TRAINING_EPOCHS: 100
TRAINING_LEARNING_RATE: 0.001
TRAINING_VALIDATION_SPLIT: 0.2
TRAINING_EARLY_STOPPING: true
TRAINING_PATIENCE: 10

# Scheduler
SCHEDULER_ENABLED: true
SCHEDULER_INTERVAL_SECONDS: 3600
SCHEDULER_MAX_CONCURRENT: 2

# API Server
API_PORT: 50053
API_HOST: 0.0.0.0

# Database (for feature store)
DATABASE_URL: postgresql://postgres:${POSTGRES_PASSWORD:-postgres}@timescaledb:5432/neural_trader

# Logging & Monitoring
LOG_LEVEL: ${LOG_LEVEL:-info}
METRICS_PORT: 9093
HEALTH_PORT: 50053
```

### 6. Neural-Trading Service

**Purpose**: Trading execution with DAA coordination

**Required Environment Variables**:
```yaml
# Service Identity
SERVICE_NAME: neural-trading
CONFIG_ENV: ${CONFIG_ENV:-dev}

# Config Store Connection
CONFIG_STORE_URL: config-store:50051

# INHERIT ALL NEURAL-CORE EVENTBUS CONFIG
# Plus override these specific channels:
EVENTBUS_CHANNELS: ["trading:signals:*", "trading:orders:*", "trading:positions:*"]

# Redis (for DAA coordination)
REDIS_URL: redis://redis:6379
REDIS_DAA_CHANNEL: daa:coordination

# Trading Configuration
TRADING_MODE: ${TRADING_MODE:-paper}  # Options: paper, live
TRADING_CAPITAL: 100000
MAX_POSITION_SIZE: 1000
RISK_LIMIT: 100
LEVERAGE: 1
COMMISSION: 0.001
SLIPPAGE: 0.0005

# Risk Management
STOP_LOSS_PCT: 2.0
TAKE_PROFIT_PCT: 5.0
POSITION_SIZING: kelly  # Options: kelly, fixed, percentage
MAX_DRAWDOWN_PCT: 10.0
DAILY_LOSS_LIMIT_PCT: 2.0

# DAA Coordinator
DAA_CONSENSUS_THRESHOLD: 0.6
DAA_VOTING_TIMEOUT_MS: 100
DAA_MAX_AGENTS: 5

# ML Model Integration
NEURAL_MODEL_PATH: /app/models/latest
MODEL_INFERENCE_TIMEOUT_MS: 50
MODEL_CACHE_SIZE: 1000

# Broker Integration
BROKER_ENDPOINT: ${BROKER_ENDPOINT}
BROKER_API_KEY: ${BROKER_API_KEY}
BROKER_API_SECRET: ${BROKER_API_SECRET}
BROKER_RETRY_ATTEMPTS: 3

# WebSocket Server
WEBSOCKET_ENABLED: true
WEBSOCKET_PORT: 8080
WEBSOCKET_PATH: /ws

# Database
DATABASE_URL: postgresql://postgres:${POSTGRES_PASSWORD:-postgres}@timescaledb:5432/neural_trader

# Logging & Monitoring  
LOG_LEVEL: ${LOG_LEVEL:-info}
METRICS_PORT: 9094
HEALTH_PORT: 50054
TRADE_LOGS_ENABLED: true
```

## Base Configuration Files

### configs/base/config-store/config.yaml
```yaml
service:
  name: config-store
  version: 1.0.0
  
server:
  grpc:
    port: 50051
    max_connections: 100
  health:
    port: 8090
    path: /health

git:
  sync_interval: 300
  branch: main
  shallow_clone: true
  
redis:
  db: 0
  cache_ttl: 3600
  key_prefix: "config:"
  
validation:
  enabled: true
  strict_mode: false
  
logging:
  level: info
  format: json
  
monitoring:
  metrics:
    enabled: true
    port: 9091
```

### configs/base/data-ingestion/config.yaml
```yaml
service:
  name: data-ingestion
  version: 1.0.0
  
api:
  port: 8081
  host: 0.0.0.0
  rate_limit: 1000
  timeout: 30
  max_connections: 100

sources:
  polygon:
    enabled: false
    rate_limit: 5
    timeout: 10
  alpha_vantage:
    enabled: false
    rate_limit: 5
    timeout: 10
  synthetic:
    enabled: true
    interval: 5

redis:
  stream: market_data_raw
  max_len: 10000
  consumer_group: data-ingestion
  
buffer:
  size: 10000
  flush_interval: 1000
  
retry:
  max_attempts: 3
  backoff_ms: 1000
  max_backoff_ms: 30000

logging:
  level: info
  format: json
  
monitoring:
  metrics:
    enabled: true
    port: 9091
  health:
    port: 8081
    path: /health
```

### configs/base/data-staging/config.yaml
```yaml
service:
  name: data-staging
  version: 1.0.0
  source_id: data-staging
  domain: trading

processing:
  transform_verbose: false
  quality_threshold: 0.7
  
redis:
  input_stream: market_data_raw
  output_stream: market_data_staged
  consumer_group: data-staging
  batch_size: 100
  block_timeout_ms: 5000
  
database:
  pool_size: 10
  timeout_seconds: 30
  
dlq:
  enabled: true
  max_retries: 3
  
eventbus:
  type: redis
  channels:
    - "market_data.*"
    - "staging.*"
  batch_size: 100
  buffer_size: 1024
  
logging:
  level: info
  format: json
  
monitoring:
  metrics:
    enabled: true
    port: 9092
  health:
    port: 50052
```

### configs/base/neural-ml-ops/config.yaml
```yaml
service:
  name: neural-ml-ops
  version: 1.0.0
  
api:
  port: 50053
  host: 0.0.0.0
  
models:
  path: /app/models
  registry_type: filesystem
  retention_days: 90
  
features:
  store_type: memory
  cache_size: 10000
  ttl_seconds: 3600
  
training:
  batch_size: 32
  epochs: 100
  learning_rate: 0.001
  validation_split: 0.2
  early_stopping: true
  patience: 10
  
scheduler:
  enabled: true
  interval_seconds: 3600
  max_concurrent: 2

# EventBus configuration inherited from neural-core
eventbus:
  type: redis
  channels:
    - "ml:features:*"
    - "ml:models:*"
    - "ml:training:*"
  batch_size: 100
  buffer_size: 1024
  block_timeout_ms: 5000
  ack_timeout_ms: 30000
  
logging:
  level: info
  format: json
  
monitoring:
  metrics:
    enabled: true
    port: 9093
  health:
    port: 50053
```

### configs/base/neural-trading/config.yaml
```yaml
service:
  name: neural-trading
  version: 1.0.0
  
trading:
  mode: paper
  capital: 100000
  max_position_size: 1000
  risk_limit: 100
  leverage: 1
  commission: 0.001
  slippage: 0.0005
  
risk:
  stop_loss_pct: 2.0
  take_profit_pct: 5.0
  position_sizing: kelly
  max_drawdown_pct: 10.0
  daily_loss_limit_pct: 2.0
  
daa:
  consensus_threshold: 0.6
  voting_timeout_ms: 100
  max_agents: 5
  redis_channel: "daa:coordination"
  
inference:
  model_path: /app/models/latest
  timeout_ms: 50
  cache_size: 1000
  
broker:
  retry_attempts: 3
  
websocket:
  enabled: true
  port: 8080
  path: /ws

# EventBus configuration inherited from neural-core  
eventbus:
  type: redis
  channels:
    - "trading:signals:*"
    - "trading:orders:*"
    - "trading:positions:*"
  batch_size: 100
  buffer_size: 1024
  block_timeout_ms: 5000
  ack_timeout_ms: 30000
  
logging:
  level: info
  format: json
  trade_logs: true
  
monitoring:
  metrics:
    enabled: true
    port: 9094
    track_trades: true
    track_pnl: true
  health:
    port: 50054
```

## Neural-Core Configuration Inheritance

### How It Works

The **neural-core** library provides shared EventBus configuration that is inherited by both **neural-ml-ops** and **neural-trading** services. This ensures consistent messaging behavior across the system.

### Implementation Pattern

```rust
// In neural-ml-ops and neural-trading
use neural_core::eventbus::{EventBus, SubscriptionConfig};

// Load base EventBus config from environment
let eventbus_config = SubscriptionConfig {
    group_name: env::var("EVENTBUS_DEFAULT_GROUP").unwrap_or("neural-trader".to_string()),
    consumer_name: format!("{}-{}", 
        env::var("SERVICE_NAME").unwrap(),
        env::var("HOSTNAME").unwrap()
    ),
    batch_size: env::var("EVENTBUS_BATCH_SIZE")
        .unwrap_or("10".to_string())
        .parse().unwrap(),
    block_timeout_ms: env::var("EVENTBUS_BLOCK_TIMEOUT_MS")
        .unwrap_or("5000".to_string())
        .parse().unwrap(),
    // ... other config
};

// Service-specific channel subscription
let channels = env::var("EVENTBUS_CHANNELS")
    .unwrap()
    .split(',')
    .map(|s| s.to_string())
    .collect();
```

## Environment Overlay Strategy

### Development (configs/overlays/dev/)
- Enable debug logging
- Disable strict validation
- Use synthetic data
- Shorter intervals
- Local resources

### Test (configs/overlays/test/)
- Production-like settings
- Enable all validations
- Test data sources
- Standard intervals
- Test databases

### Production (configs/overlays/prod/)
- Info logging only
- Strict validation
- Real data sources
- Optimized intervals
- Production resources

## Critical Configuration Notes

### 1. Service Dependencies
- **All services** depend on `config-store` being healthy
- **data-staging** depends on `data-ingestion`
- **neural-ml-ops** depends on `data-staging`
- **neural-trading** depends on `neural-ml-ops`

### 2. Shared Resources
- **Redis**: Used by all services for different purposes
  - Config caching (config-store)
  - Data streaming (data-ingestion, data-staging)
  - EventBus (neural-core, ml-ops, trading)
  - DAA coordination (neural-trading)
- **TimescaleDB**: Used by data-staging, ml-ops, and trading

### 3. Security Considerations
- API keys should NEVER be in base configs
- Use `.env.${CONFIG_ENV}` files for secrets
- Implement token-based auth for config-store (future)

### 4. Performance Tuning
- Adjust batch sizes based on load
- Configure timeouts for network conditions
- Set appropriate buffer sizes
- Tune connection pools

## Validation Checklist

- [ ] All services have `SERVICE_NAME` environment variable
- [ ] All services have `CONFIG_STORE_URL` pointing to config-store
- [ ] Redis URL is consistent across services
- [ ] Database URL is consistent where used
- [ ] EventBus configuration is properly inherited
- [ ] Health check ports don't conflict
- [ ] Metrics ports are unique per service
- [ ] Log levels are appropriate per environment
- [ ] Secrets are in .env files, not configs
- [ ] Channel names follow naming conventions

## Recommendations

1. **Immediate Actions**
   - Update all base configuration files as specified above
   - Ensure docker-compose.v2.yml has all required environment variables
   - Create .env.dev, .env.test, and .env.prod templates

2. **Configuration Validation**
   - Implement schema validation for each service config
   - Add startup checks for required environment variables
   - Create health check endpoints that verify configuration

3. **Neural-Core Integration**
   - Ensure neural-ml-ops includes neural-core dependency
   - Ensure neural-trading includes neural-core dependency
   - Standardize EventBus initialization across services

4. **Monitoring**
   - Ensure unique metrics ports (9091-9094)
   - Configure Prometheus scraping for all services
   - Set up Grafana dashboards for each service

---

*Generated: 2025-08-29*
*Status: READY FOR IMPLEMENTATION*
*Priority: CRITICAL - Required for service startup*