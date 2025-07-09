# Configuration Examples for Autonomous Neural Trader

## Table of Contents
1. [Basic Configuration](#basic-configuration)
2. [Neural Network Configuration](#neural-network-configuration)
3. [DAA Configuration](#daa-configuration)
4. [Trading Strategy Configuration](#trading-strategy-configuration)
5. [Data Source Configuration](#data-source-configuration)
6. [Production Configuration](#production-configuration)

## Basic Configuration

### Minimal Setup (config/trading.yaml)
```yaml
# Basic trading configuration
trading:
  mode: paper  # paper or live
  symbols:
    - AAPL
    - GOOGL
  strategies:
    - neural_enhanced
  
  risk_limits:
    max_position_size: 1000
    max_daily_loss: 500
    stop_loss_percentage: 0.02
    
  execution:
    order_type: market
    time_in_force: day
```

### Environment Variables (.env)
```bash
# Core Settings
RUST_LOG=info
ENVIRONMENT=development

# API Keys
ALPHA_VANTAGE_API_KEY=your_key_here
YAHOO_FINANCE_API_KEY=your_key_here
FINNHUB_API_KEY=your_key_here

# Database
DATABASE_URL=postgresql://user:pass@localhost:5432/neural_trader
REDIS_URL=redis://localhost:6379

# Neural Network
NEURAL_MODEL_PATH=/models
NEURAL_TRAINING_ENABLED=true
NEURAL_UPDATE_INTERVAL=3600

# DAA Settings
DAA_ENABLED=true
DAA_AGENT_COUNT=5
```

## Neural Network Configuration

### FANN Configuration (config/neural.toml)
```toml
[neural.fann]
# Network architecture
layers = [20, 40, 40, 20, 1]
learning_rate = 0.001
momentum = 0.9
training_epochs = 1000

# Data preprocessing
normalize_inputs = true
input_window = 50
prediction_horizon = "5min"

# Training settings
batch_size = 32
validation_split = 0.2
early_stopping_patience = 10

# Model persistence
save_interval = 3600
model_directory = "/models/fann"
```

### Neuro-Divergent Models (config/models.yaml)
```yaml
neural_models:
  nhits:
    enabled: true
    config:
      horizon: 24
      input_size: 100
      sampling_rates: [1, 2, 4, 8]
      mlp_units:
        - [512, 512]
        - [512, 512]
        - [512, 512]
        - [512, 512]
      dropout: 0.1
      
  tcn:
    enabled: true
    config:
      horizon: 24
      num_filters: 32
      num_layers: 8
      kernel_size: 3
      dilation_base: 2
      dropout: 0.1
      
  deepar:
    enabled: true
    config:
      horizon: 24
      hidden_size: 64
      num_layers: 2
      distribution: gaussian
      num_samples: 100
      dropout: 0.1
```

## DAA Configuration

### Agent Configuration (config/agents.yaml)
```yaml
daa:
  # Consensus settings
  consensus:
    method: weighted_voting
    threshold: 0.7
    timeout_seconds: 5
    require_minimum_agents: 3
    
  # Agent spawn configuration
  agents:
    auto_spawn: true
    initial_agents:
      - type: risk_analyst
        count: 2
        config:
          risk_tolerance: conservative
          max_drawdown: 0.1
          
      - type: technical_analyst
        count: 1
        config:
          indicators:
            - rsi
            - macd
            - bollinger_bands
          timeframes: ["1m", "5m", "15m"]
          
      - type: fundamental_analyst
        count: 1
        config:
          metrics:
            - pe_ratio
            - revenue_growth
            - debt_to_equity
            
      - type: sentiment_analyst
        count: 1
        config:
          sources:
            - news
            - social_media
          sentiment_threshold: 0.6
          
  # Learning configuration
  learning:
    enabled: true
    feedback_delay_seconds: 300
    min_samples_for_update: 50
    performance_tracking: true
    
  # Risk management
  risk_management:
    position_sizing:
      method: kelly_criterion
      max_allocation: 0.25
    diversification:
      max_correlation: 0.7
      min_assets: 3
```

## Trading Strategy Configuration

### Neural Enhanced Strategy (config/strategies/neural_enhanced.yaml)
```yaml
neural_enhanced:
  # Base configuration
  enabled: true
  allocation: 0.4  # 40% of portfolio
  
  # Neural model settings
  models:
    primary: fann
    ensemble:
      - nhits
      - tcn
      - deepar
    voting_method: weighted_average
    
  # Entry conditions
  entry:
    prediction_threshold: 0.02  # 2% expected gain
    confidence_threshold: 0.7
    volume_confirmation: true
    trend_alignment: true
    
  # Exit conditions  
  exit:
    take_profit: 0.05  # 5%
    stop_loss: 0.02    # 2%
    trailing_stop:
      enabled: true
      distance: 0.01   # 1%
    time_stop:
      enabled: true
      max_hours: 24
      
  # Risk controls
  risk:
    max_position_size: 5000
    max_concurrent_positions: 5
    sector_concentration_limit: 0.3
    
  # DAA integration
  daa:
    require_consensus: true
    min_agreement: 0.6
    veto_agents:
      - risk_analyst
```

### Momentum Strategy (config/strategies/momentum.yaml)
```yaml
momentum:
  enabled: true
  allocation: 0.3
  
  # Momentum calculation
  lookback_period: 20
  rebalance_frequency: daily
  
  # Selection criteria
  selection:
    top_n: 10
    minimum_momentum: 0.05
    volume_filter:
      min_average_volume: 1000000
      volume_surge_multiplier: 1.5
      
  # Position management
  positions:
    equal_weight: false
    weight_by_momentum: true
    max_position: 0.1
    
  # Risk management
  risk:
    stop_loss: 0.03
    correlation_limit: 0.5
    volatility_scaling: true
```

## Data Source Configuration

### Market Data Sources (config/data_sources.yaml)
```yaml
data_sources:
  # Yahoo Finance
  yahoo_finance:
    enabled: true
    priority: 1
    symbols: ["AAPL", "GOOGL", "MSFT", "AMZN"]
    intervals: ["1m", "5m", "15m", "1h", "1d"]
    rate_limit:
      requests_per_minute: 60
      retry_attempts: 3
      
  # Alpha Vantage
  alpha_vantage:
    enabled: true
    priority: 2
    api_key: ${ALPHA_VANTAGE_API_KEY}
    endpoints:
      - TIME_SERIES_INTRADAY
      - GLOBAL_QUOTE
      - TECHNICAL_INDICATORS
    rate_limit:
      requests_per_minute: 5
      
  # Finnhub
  finnhub:
    enabled: true
    priority: 3
    api_key: ${FINNHUB_API_KEY}
    streams:
      - trades
      - quotes
      - news
    websocket:
      enabled: true
      reconnect_interval: 5
      
  # Data aggregation
  aggregation:
    method: weighted_average
    outlier_detection: true
    missing_data_strategy: forward_fill
    cache_ttl_seconds: 60
```

### Data Pipeline (config/pipeline.yaml)
```yaml
data_pipeline:
  # Ingestion settings
  ingestion:
    batch_size: 1000
    parallel_workers: 4
    buffer_size: 10000
    
  # Processing stages
  processing:
    - stage: validation
      enabled: true
      rules:
        - price_range: [0.01, 100000]
        - volume_minimum: 0
        - timestamp_validation: true
        
    - stage: normalization
      enabled: true
      methods:
        - price: log_transform
        - volume: min_max_scaling
        - indicators: z_score
        
    - stage: feature_engineering
      enabled: true
      features:
        - moving_averages: [5, 10, 20, 50]
        - rsi_periods: [14]
        - volatility_window: 20
        
  # Storage
  storage:
    timescale:
      retention_days: 30
      compression: true
      partitioning: daily
    redis:
      ttl_seconds: 300
      max_memory: "1gb"
```

## Production Configuration

### Docker Compose Override (docker-compose.prod.yml)
```yaml
version: '3.8'

services:
  neural-trader:
    image: neural-trader:latest
    environment:
      - ENVIRONMENT=production
      - RUST_LOG=warn
      - NEURAL_TRAINING_ENABLED=false
      - DAA_AGENT_COUNT=10
    deploy:
      replicas: 2
      resources:
        limits:
          cpus: '4'
          memory: 8G
        reservations:
          cpus: '2'
          memory: 4G
    volumes:
      - ./config/production:/config:ro
      - neural-models:/models
      
  redis:
    command: redis-server --appendonly yes --requirepass ${REDIS_PASSWORD}
    volumes:
      - redis-data:/data
      
  timescaledb:
    environment:
      - POSTGRES_PASSWORD=${DB_PASSWORD}
      - POSTGRES_SSL_MODE=require
    volumes:
      - timescale-data:/var/lib/postgresql/data
      
  prometheus:
    volumes:
      - ./config/prometheus-prod.yml:/etc/prometheus/prometheus.yml:ro
      - prometheus-data:/prometheus
      
volumes:
  neural-models:
  redis-data:
  timescale-data:
  prometheus-data:
```

### Security Configuration (config/security.yaml)
```yaml
security:
  # API Security
  api:
    authentication:
      method: jwt
      token_expiry: 3600
      refresh_enabled: true
    rate_limiting:
      enabled: true
      requests_per_minute: 100
      burst_size: 20
    cors:
      enabled: true
      allowed_origins:
        - https://trading.example.com
      
  # Database Security
  database:
    encryption_at_rest: true
    ssl_mode: require
    connection_pool:
      max_connections: 20
      idle_timeout: 300
      
  # Redis Security
  redis:
    requirepass: true
    tls_enabled: true
    maxmemory_policy: volatile-lru
    
  # Secrets Management
  secrets:
    provider: vault
    vault_address: https://vault.example.com
    mount_path: /secret/neural-trader
    auto_renew: true
```

### Monitoring Configuration (config/monitoring.yaml)
```yaml
monitoring:
  # Metrics
  metrics:
    enabled: true
    port: 9090
    path: /metrics
    
  # Alerts
  alerts:
    - name: high_loss_rate
      condition: loss_rate > 0.1
      severity: critical
      notification:
        - email: alerts@example.com
        - slack: trading-alerts
        
    - name: model_accuracy_drop
      condition: prediction_accuracy < 0.6
      severity: warning
      
    - name: consensus_timeout
      condition: daa_consensus_timeouts > 5
      severity: warning
      
  # Logging
  logging:
    level: info
    format: json
    outputs:
      - stdout
      - file: /var/log/neural-trader/app.log
    rotation:
      max_size: 100M
      max_age: 7d
      compress: true
      
  # Tracing
  tracing:
    enabled: true
    provider: jaeger
    endpoint: http://jaeger:14268/api/traces
    sample_rate: 0.1
```

### High Availability Configuration (config/ha.yaml)
```yaml
high_availability:
  # Clustering
  clustering:
    enabled: true
    mode: active-active
    nodes:
      - node1.trading.local
      - node2.trading.local
      - node3.trading.local
    
  # Load balancing
  load_balancing:
    method: round_robin
    health_check:
      interval: 5s
      timeout: 2s
      threshold: 3
      
  # Failover
  failover:
    automatic: true
    detection_time: 10s
    switchover_time: 5s
    
  # Data replication
  replication:
    redis:
      mode: sentinel
      replicas: 2
    postgres:
      mode: streaming
      standby_nodes: 2
```

## Usage Examples

### Development Setup
```bash
# Use development configuration
cp config/development.toml config/active.toml
export ENVIRONMENT=development
cargo run
```

### Production Deployment
```bash
# Use production configuration
cp config/production.toml config/active.toml
export ENVIRONMENT=production

# Deploy with Docker
docker-compose -f docker-compose.yml -f docker-compose.prod.yml up -d

# Scale neural trader service
docker-compose scale neural-trader=3
```

### Testing Configuration
```bash
# Use test configuration with mocked data sources
export ENVIRONMENT=test
export MOCK_DATA_SOURCES=true
cargo test --features integration
```

These configuration examples demonstrate how to set up the autonomous neural trader for different scenarios, from development to production deployment.