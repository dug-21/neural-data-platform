# Configuration Guide

This guide covers all configuration options for the Neural Trading Platform. The system uses a hierarchical configuration approach that allows for flexible deployment across different environments.

## 🎯 Configuration Philosophy

The platform follows these configuration principles:
- **Environment-based**: Different settings for development, testing, and production
- **Secure by Default**: Sensitive information via environment variables
- **Validation First**: All configuration validated at startup
- **Hot Reload**: Some settings can be updated without restart

## 📁 Configuration Structure

```
neural-trader/
├── .env                          # Environment variables (you create this)
├── .env.example                  # Template for environment setup
├── config/
│   ├── platform.toml            # Base platform configuration
│   ├── development.toml         # Development overrides
│   ├── production.toml          # Production settings
│   ├── test.toml               # Test environment
│   └── trading.yaml            # Trading strategy configuration
├── neural-trader-config/        # Extended configuration
│   ├── agents.yaml             # DAA agent configuration
│   ├── autonomous_training.toml # Neural training settings
│   └── data_requirements.toml  # Data provider requirements
└── docker-compose.yml          # Container orchestration
```

## 🚀 Quick Configuration

### Minimal Setup (.env)
For a basic setup, create a `.env` file with:

```bash
# Database
POSTGRES_USER=neural_trader
POSTGRES_PASSWORD=secure_password_here
POSTGRES_DB=neural_trader_db

# Redis
REDIS_PASSWORD=redis_password_here

# Primary Data Provider (choose one)
ALPACA_API_KEY=your_alpaca_key
ALPACA_API_SECRET=your_alpaca_secret
ALPACA_PAPER_TRADING=true

# Trading
TRADING_SYMBOLS_PRIMARY=AAPL,MSFT,GOOGL,TSLA,NVDA
PRIMARY_PROVIDER=alpaca
USE_PAPER_TRADING=true
```

### Complete Setup (.env)
For full functionality with multiple providers:

```bash
#========================================
# DATABASE CONFIGURATION
#========================================
POSTGRES_USER=neural_trader
POSTGRES_PASSWORD=secure_database_password
POSTGRES_DB=neural_trader_db
POSTGRES_HOST=timescaledb
POSTGRES_PORT=5432

#========================================
# REDIS CONFIGURATION
#========================================
REDIS_HOST=redis
REDIS_PORT=6379
REDIS_PASSWORD=secure_redis_password
REDIS_DB=0

#========================================
# MARKET DATA PROVIDERS
#========================================

# Alpaca Markets (Primary - recommended for beginners)
ALPACA_API_KEY=your_alpaca_api_key
ALPACA_API_SECRET=your_alpaca_api_secret
ALPACA_WS_ENABLED=true
ALPACA_PAPER_TRADING=true  # Set to false for live trading

# Polygon (Professional real-time data)
POLYGON_API_KEY=your_polygon_api_key
POLYGON_WS_ENABLED=true

# Finnhub (Comprehensive coverage)
FINNHUB_API_KEY=your_finnhub_api_key

# IEX Cloud (Institutional grade)
IEX_CLOUD_API_KEY=your_iex_api_key
IEX_CLOUD_SANDBOX=true  # Set to false for production

# Alpha Vantage (Technical indicators)
ALPHA_VANTAGE_API_KEY=your_alpha_vantage_key

#========================================
# TRADING CONFIGURATION
#========================================
TRADING_SYMBOLS_PRIMARY=AAPL,MSFT,GOOGL,TSLA,NVDA,AMZN,META,NFLX
TRADING_SYMBOLS_SECONDARY=SPY,QQQ,IWM,GLD,SLV
PRIMARY_PROVIDER=alpaca
FALLBACK_PROVIDERS=finnhub,alpha_vantage
USE_PAPER_TRADING=true
MAX_POSITION_SIZE=0.02  # 2% of portfolio per position
STOP_LOSS_PERCENT=0.02  # 2% stop loss
TAKE_PROFIT_PERCENT=0.05  # 5% take profit

#========================================
# NEURAL NETWORK CONFIGURATION
#========================================
NEURAL_MEMORY_GB=2.0
NEURAL_MODELS=NHITS,TCN,DeepAR,Transformer,MLP
NEURAL_PREDICTION_CACHE_TTL=300
NEURAL_TRAINING_ENABLED=true
NEURAL_ONLINE_LEARNING=true

#========================================
# MONITORING & ALERTING
#========================================
GRAFANA_ADMIN_PASSWORD=secure_grafana_password
PROMETHEUS_RETENTION_TIME=30d
ALERT_EMAIL=your-email@example.com
SLACK_WEBHOOK_URL=your_slack_webhook_url

#========================================
# SECURITY SETTINGS
#========================================
JWT_SECRET=your_jwt_secret_key_here
API_RATE_LIMIT=1000  # Requests per minute
ENABLE_AUDIT_LOGGING=true
LOG_LEVEL=INFO

#========================================
# PERFORMANCE TUNING
#========================================
WORKER_THREADS=4
DATABASE_MAX_CONNECTIONS=20
REDIS_MAX_CONNECTIONS=10
NEURAL_BATCH_SIZE=100
```

## 🏗️ Platform Configuration (TOML)

### Base Configuration (config/platform.toml)
```toml
[platform]
name = "neural-trader-autonomous"
version = "0.1.0"
environment = "development"

[database]
url = "postgres://neural_trader:password@timescaledb/neural_trader_db"
max_connections = 20
idle_timeout_secs = 300
connect_timeout_secs = 30

[redis]
url = "redis://redis:6379"
password = "${REDIS_PASSWORD}"
default_ttl_seconds = 3600
max_connections = 10

[neural]
memory_gb = 1.0
models = ["NHITS", "DeepAR", "TCN", "MLP"]
prediction_cache_ttl = 300
training_enabled = true
online_learning = true
batch_size = 50

[monitoring]
metrics_interval_secs = 60
quality_threshold = 0.95
enable_prometheus = true
enable_grafana = true

[logging]
level = "INFO"
structured = true
file_rotation = true
max_file_size_mb = 100
max_files = 10
```

### Production Overrides (config/production.toml)
```toml
[platform]
environment = "production"

[database]
max_connections = 50
idle_timeout_secs = 600

[redis]
max_connections = 20

[neural]
memory_gb = 4.0
batch_size = 100
models = ["NHITS", "DeepAR", "TCN", "Transformer", "MLP"]

[monitoring]
metrics_interval_secs = 30
enable_detailed_metrics = true

[logging]
level = "WARN"
enable_audit = true
```

## 🤖 Trading Strategy Configuration

### Trading Strategies (config/trading.yaml)
```yaml
strategies:
  - name: momentum
    enabled: true
    weight: 0.3
    parameters:
      lookback_period: 20
      momentum_threshold: 0.02
      risk_limit: 0.02
      position_size: 0.1
    
  - name: neural_enhanced
    enabled: true
    weight: 0.5
    parameters:
      confidence_threshold: 0.75
      ensemble_weight: 0.6
      momentum_weight: 0.4
      risk_limit: 0.02
      position_size: 0.1
      
  - name: mean_reversion
    enabled: false
    weight: 0.2
    parameters:
      bollinger_periods: 20
      bollinger_std: 2.0
      rsi_periods: 14
      rsi_oversold: 30
      rsi_overbought: 70

risk_management:
  max_portfolio_risk: 0.10      # 10% maximum portfolio risk
  max_position_size: 0.02       # 2% maximum per position
  max_correlation: 0.7          # Maximum correlation between positions
  stop_loss_percent: 0.02       # 2% stop loss
  take_profit_percent: 0.05     # 5% take profit
  max_daily_trades: 50          # Maximum trades per day
  max_open_positions: 10        # Maximum concurrent positions

market_hours:
  timezone: "America/New_York"
  trading_start: "09:30"
  trading_end: "16:00"
  pre_market_start: "04:00"
  after_hours_end: "20:00"
  enable_extended_hours: false
```

## 🧠 Neural Network Configuration

### Neural Models (neural-trader-config/autonomous_training.toml)
```toml
[models.nhits]
enabled = true
architecture = [128, 64, 32, 16]
lookback_periods = 50
horizon = 5
learning_rate = 0.001
batch_size = 32
epochs = 100

[models.tcn]
enabled = true
architecture = [96, 48, 24]
lookback_periods = 40
kernel_size = 3
dilation_factor = 2
learning_rate = 0.0005

[models.deepar]
enabled = true
architecture = [100, 50, 25]
lookback_periods = 60
output_distribution = "gaussian"
learning_rate = 0.001

[models.transformer]
enabled = true
architecture = [256, 128, 64, 32]
lookback_periods = 80
num_heads = 8
num_layers = 4
learning_rate = 0.0001

[models.mlp]
enabled = true
architecture = [64, 32, 16]
lookback_periods = 30
dropout_rate = 0.2
learning_rate = 0.001

[ensemble]
weighting_strategy = "performance"
confidence_threshold = 0.7
update_frequency_minutes = 60
performance_window_days = 7

[training]
retrain_frequency_hours = 24
min_samples_for_training = 1000
validation_split = 0.2
early_stopping_patience = 10
```

## 📊 Data Provider Configuration

### Provider Priorities (neural-trader-config/data_requirements.toml)
```toml
[providers]
primary = "alpaca"
fallback_order = ["finnhub", "alpha_vantage", "yahoo_finance"]

[providers.alpaca]
enabled = true
priority = 1
websocket_enabled = true
rate_limit_per_minute = 200
retry_attempts = 3
timeout_seconds = 30
paper_trading = true

[providers.polygon]
enabled = true
priority = 2
websocket_enabled = true
rate_limit_per_minute = 5000
subscription_tier = "basic"

[providers.finnhub]
enabled = true
priority = 3
rate_limit_per_minute = 60
websocket_enabled = false

[providers.iex_cloud]
enabled = false
priority = 4
rate_limit_per_minute = 100
sandbox = true

[providers.alpha_vantage]
enabled = true
priority = 5
rate_limit_per_minute = 5
rate_limit_per_day = 500

[data_quality]
min_providers_for_consensus = 2
max_price_deviation_percent = 5.0
stale_data_threshold_seconds = 60
outlier_detection_enabled = true
```

## 🚨 Security Configuration

### Security Settings
```bash
# JWT Authentication
JWT_SECRET=your-256-bit-secret-key-here
JWT_EXPIRY_HOURS=24

# API Security
API_RATE_LIMIT=1000  # requests per minute
API_BURST_LIMIT=100  # burst requests
ENABLE_CORS=false
ALLOWED_ORIGINS=https://yourdomain.com

# Network Security
ENABLE_TLS=true
TLS_CERT_PATH=/etc/ssl/certs/neural-trader.crt
TLS_KEY_PATH=/etc/ssl/private/neural-trader.key

# Audit & Compliance
ENABLE_AUDIT_LOGGING=true
AUDIT_LOG_RETENTION_DAYS=90
ENABLE_TRADE_LOGGING=true
COMPLIANCE_MODE=true
```

## 🔧 Environment-Specific Configurations

### Development Environment
```bash
# Development specific settings
LOG_LEVEL=DEBUG
ENABLE_HOT_RELOAD=true
MOCK_TRADING=true
USE_SAMPLE_DATA=true
DISABLE_RATE_LIMITS=true
```

### Testing Environment
```bash
# Testing specific settings
TESTING_MODE=true
USE_MOCK_PROVIDERS=true
ACCELERATED_TIME=true
DETERMINISTIC_RANDOM=true
LOG_LEVEL=DEBUG
```

### Production Environment
```bash
# Production specific settings
LOG_LEVEL=WARN
ENABLE_PERFORMANCE_MONITORING=true
STRICT_VALIDATION=true
ENABLE_CIRCUIT_BREAKERS=true
HIGH_AVAILABILITY_MODE=true
```

## 🔄 Configuration Validation

### Validation Rules
The system validates configuration at startup:

```rust
// Example validation rules
- Database connection must be successful
- Redis connection must be established
- At least one data provider must be configured
- Trading symbols must be valid
- Risk limits must be reasonable (0.01% - 10%)
- Neural models must have valid architectures
```

### Validation Errors
Common validation issues and solutions:

| Error | Cause | Solution |
|-------|-------|----------|
| Database connection failed | Wrong credentials/host | Check POSTGRES_* variables |
| Redis connection failed | Wrong password/host | Check REDIS_* variables |
| Invalid API key | Wrong/expired API key | Update provider API keys |
| Invalid trading symbols | Typo in symbol names | Check symbol spelling |
| Risk limits too high | Unsafe risk settings | Reduce position/risk limits |

## 🔄 Runtime Configuration Updates

Some settings can be updated without restart:

### Hot-Reloadable Settings
- Trading strategy parameters
- Risk management limits
- Logging levels
- Monitoring intervals
- Neural model weights

### Restart Required Settings
- Database connections
- Provider API keys
- Neural model architectures
- Security settings
- Network configurations

### Configuration API
```bash
# Update trading parameters
curl -X POST http://localhost:8080/api/config/trading \
  -H "Content-Type: application/json" \
  -d '{"max_position_size": 0.01}'

# Update risk limits
curl -X POST http://localhost:8080/api/config/risk \
  -H "Content-Type: application/json" \
  -d '{"stop_loss_percent": 0.015}'
```

## 📚 Configuration Best Practices

### Security Best Practices
1. **Never commit secrets**: Use environment variables only
2. **Rotate keys regularly**: Update API keys and passwords monthly
3. **Use strong passwords**: Minimum 16 characters with mixed case
4. **Enable audit logging**: Track all configuration changes
5. **Validate inputs**: All user inputs must be validated

### Performance Best Practices
1. **Tune connection pools**: Match your workload requirements
2. **Configure timeouts**: Prevent hanging operations
3. **Set appropriate batch sizes**: Balance memory vs. throughput
4. **Monitor resource usage**: Adjust limits based on actual usage
5. **Use caching wisely**: Cache frequently accessed data

### Operational Best Practices
1. **Environment parity**: Keep dev/staging/prod configs similar
2. **Document changes**: Comment non-obvious configuration choices
3. **Version control**: Track configuration changes in git
4. **Test configurations**: Validate config changes in staging first
5. **Monitor alerts**: Set up alerts for configuration drift

## 🚨 Troubleshooting Configuration

### Common Issues

#### Services Won't Start
```bash
# Check configuration syntax
docker-compose config

# Validate environment variables
env | grep -E "(POSTGRES|REDIS|ALPACA)"

# Check file permissions
ls -la .env config/
```

#### Invalid Configuration Values
```bash
# Check logs for validation errors
docker-compose logs neural-trader | grep -i "config"

# Validate TOML syntax
cat config/platform.toml | docker run --rm -i alpine/toml-validator
```

#### Performance Issues
```bash
# Check resource limits
docker stats

# Monitor configuration impact
curl http://localhost:8080/metrics | grep config
```

### Getting Help

For configuration issues:
1. Check the [Troubleshooting Guide](troubleshooting.md)
2. Review error logs for validation messages
3. Consult the [API Reference](api-reference.md) for valid values
4. Ask in [GitHub Discussions](https://github.com/yourusername/neural-trader/discussions)

---

**Next Steps**: After configuration, see the [Deployment Guide](deployment.md) for production setup or [Monitoring Guide](monitoring.md) for observability setup.