# Neural Trader Configuration Guide

## Overview

The Neural Trader system uses a hierarchical configuration system with multiple TOML files controlling different aspects of the platform. This guide documents all configuration files, their settings, and their impact on system behavior.

## Table of Contents

1. [Configuration File Hierarchy](#configuration-file-hierarchy)
2. [Core Configuration Files](#core-configuration-files)
3. [Environment Variables](#environment-variables)
4. [Configuration Loading Order](#configuration-loading-order)
5. [Settings Reference](#settings-reference)

## Configuration File Hierarchy

```
config/
├── platform.toml              # Main platform configuration
├── development.toml           # Development environment overrides
├── test.toml                  # Test environment overrides
├── production.toml            # Production environment overrides
├── sector_models.toml         # Sector-based model architecture (Phase 2)
├── autonomous_training.toml   # Autonomous training settings (Phase 3)
└── data_requirements.toml     # Data discovery configuration (Phase 3)
```

## Core Configuration Files

### 1. `platform.toml` - Main Platform Configuration

The primary configuration file that defines core system settings.

#### Sections:

##### `[platform]`
Basic platform metadata.
```toml
name = "neural-trader-autonomous"  # Platform identifier
version = "0.1.0"                  # Current version
```

##### `[database]`
PostgreSQL/TimescaleDB connection settings.
```toml
url = "postgres://user:pass@host/db"  # Connection string
max_connections = 20                  # Maximum pool connections
min_connections = 5                   # Minimum pool connections
```
**Controls**: Data persistence, time-series storage, historical data access

##### `[redis]`
Redis cache and pub/sub configuration.
```toml
url = "redis://localhost:6379"        # Redis connection
max_connections = 10                  # Connection pool size
default_ttl_seconds = 3600           # Default cache TTL

[redis.channels]
market_data_patterns = ["market:*"]   # Channel patterns to subscribe
sector_channels = ["sector:*:*"]      # Sector aggregation channels
enable_channel_discovery = true       # Dynamic channel discovery
```
**Controls**: Real-time data streaming, caching, inter-process communication

##### `[neural]`
Neural network configuration and model selection.
```toml
memory_gb = 1.0                      # Memory allocation for models
models = ["NHITS", "DeepAR", "TCN"]  # Active vendor models
use_real_models = true               # Use vendor models (not FANN)
prediction_cache_ttl = 300           # Cache predictions for 5 minutes

[neural.sector_configuration]
enabled = true                       # Enable sector-based architecture
config_path = "config/sector_models.toml"  # Sector config location
memory_target_mb_per_symbol = 50     # Phase 2 memory target
shared_feature_extraction = true     # Enable feature sharing

[neural.model_selection]
vendor_models_only = true            # Only use vendor models
model_weights = { NHITS = 1.2, TCN = 1.1 }  # Model importance weights
```
**Controls**: Model architecture, memory usage, prediction behavior

##### `[daa]`
Decentralized Autonomous Agents configuration.
```toml
enabled = true
neural_weight = 0.6                  # 60% weight for neural predictions
strategy_weight = 0.4                # 40% weight for strategy signals
consensus_threshold = 0.7            # 70% Byzantine consensus
enable_sector_coordinators = true    # Hierarchical coordination
```
**Controls**: Autonomous trading decisions, voting mechanisms, risk management

##### `[monitoring]`
System monitoring and health checks.
```toml
metrics_interval_secs = 60           # Metrics collection interval
quality_threshold = 0.95             # Data quality threshold
```
**Controls**: System observability, health monitoring, alerting

##### `[autonomous_training]`
Reference to autonomous training configuration.
```toml
enabled = true
config_path = "config/autonomous_training.toml"
```
**Controls**: Enables the autonomous training subsystem

##### `[data_discovery]`
Reference to data requirements configuration.
```toml
enabled = true
config_path = "config/data_requirements.toml"
```
**Controls**: Dynamic data type discovery system

##### `[training_service]`
Training service integration settings.
```toml
enabled = true
batch_size = 32                      # Training batch size
validation_split = 0.2               # Validation data percentage
early_stopping_patience = 5          # Epochs before stopping
```
**Controls**: Model training behavior, validation, resource usage

### 2. `sector_models.toml` - Sector Architecture Configuration

Defines the 10-sector clustering system for Phase 2.

#### Key Sections:

##### `[sectors.*]`
Individual sector definitions (technology, financial_services, etc.)
```toml
[sectors.technology]
etf_representative = "XLK"           # Sector ETF for validation
symbols = ["AAPL", "MSFT", "GOOGL"]  # Symbols in this sector
shared_memory_mb = 512               # Shared memory allocation
specialization_memory_mb = 8         # Per-symbol specialization
correlation_threshold = 0.65         # Intra-sector correlation limit
```
**Controls**: Symbol-to-sector mapping, memory allocation, correlation limits

##### `[models.*]`
Model configurations per sector.
```toml
[models.technology_lstm]
model_type = "LSTM"
sector = "technology"
required_data = ["price", "volume"]
optional_data = ["sentiment", "news"]
max_memory_mb = 256
min_accuracy = 0.78
ensemble_weight = 0.35
```
**Controls**: Model selection, data requirements, performance thresholds

##### `[daa_coordination]`
Hierarchical DAA settings.
```toml
[daa_coordination.master_coordinator]
portfolio_consensus_threshold = 0.70
cross_sector_risk_weight = 0.30
max_portfolio_positions = 20

[daa_coordination.sector_coordinators]
sector_consensus_threshold = 0.65
max_sector_positions = 4
```
**Controls**: Multi-level decision making, risk distribution, position limits

##### `[performance]`
Performance optimization settings.
```toml
[performance.memory_optimization]
enable_shared_features = true
feature_cache_ttl_seconds = 300
memory_pressure_threshold = 0.85

[performance.accuracy_thresholds]
min_sector_accuracy = 0.70
min_symbol_accuracy = 0.65
```
**Controls**: Memory efficiency, caching behavior, quality thresholds

### 3. `autonomous_training.toml` - Autonomous Training Configuration

Controls the DAA autonomous training system (Phase 3).

#### Key Sections:

##### `[autonomous_training]`
Core training system settings.
```toml
enabled = true
accuracy_threshold = 0.8             # Trigger retraining below this
error_rate_threshold = 0.1           # Maximum acceptable error rate
consecutive_failure_threshold = 5     # Failures before rollback
checkpoint_interval_minutes = 30      # Model checkpoint frequency
```
**Controls**: When and how autonomous training occurs

##### `[realtime_training]`
Real-time adaptive training (Phase 3).
```toml
enabled = true
online_learning_rate = 0.001
gradient_update_latency_ms = 50      # Real-time update speed
adaptive_learning_rates = true       # Dynamic learning rate adjustment
market_regime_detection = true       # Adapt to market conditions
```
**Controls**: Real-time model adaptation, online learning behavior

##### `[training_scheduler]`
Market-aware training scheduling.
```toml
market_aware_scheduling = true
resource_limit_during_market = 0.3   # 30% CPU during market hours
resource_limit_after_hours = 0.8     # 80% CPU after hours
preferred_training_window = "02:00-06:00"  # UTC training window
```
**Controls**: When training occurs, resource allocation

##### `[resource_management]`
Training resource limits.
```toml
max_concurrent_training = 2
memory_limit_gb = 2.0
cpu_cores_limit = 4
```
**Controls**: Resource consumption, parallel training capacity

### 4. `data_requirements.toml` - Data Discovery Configuration

Defines data characteristics for dynamic discovery (Phase 3).

#### Key Sections:

##### `[data_discovery]`
Dynamic data type discovery settings.
```toml
enabled = true
dynamic_type_registration = true     # Runtime type discovery
channel_agnostic = true             # Work with any channel pattern
supported_scopes = ["symbol", "market", "sector", "geographic"]
```
**Controls**: How new data types are discovered and registered

##### `[characteristic_requirements.*]`
Data requirements by characteristics (not specific types).
```toml
[characteristic_requirements.high_frequency]
min_update_frequency_seconds = 60
max_latency_ms = 100
quality_threshold = 0.95
```
**Controls**: Model activation based on data characteristics

##### `[data_routing]`
Multi-scope data routing configuration.
```toml
[data_routing.symbol_specific]
channel_patterns = ["symbol:*:*"]
priority = 1

[data_routing.sector_specific]
channel_patterns = ["sector:*:*"]
map_to_symbols = true
```
**Controls**: How data is routed from channels to models

##### `[model_activation]`
Automatic model activation rules.
```toml
[model_activation.rules]
basic_models = { required_chars = ["high_frequency"] }
advanced_models = { required_chars = ["high_frequency", "medium_frequency"] }
```
**Controls**: Which models activate based on available data

### 5. Environment-Specific Configurations

#### `development.toml`
Overrides for development environment:
- Lower resource limits
- More verbose logging
- Disabled features for testing
- Local service endpoints

#### `test.toml`
Overrides for test environment:
- In-memory databases
- Mocked external services
- Faster timeouts
- Deterministic settings

#### `production.toml`
Overrides for production environment:
- Optimized resource allocation
- Production service endpoints
- Security settings
- Performance optimizations

## Environment Variables

Critical environment variables that override configuration:

```bash
# Feature Flags
ENABLE_AUTONOMOUS_TRAINING=true      # Enable autonomous training system
ENABLE_SECTOR_MODELS=true           # Enable sector-based architecture
ENABLE_REALTIME_ADAPTATION=true     # Enable real-time model updates
ENABLE_DATA_DISCOVERY=true          # Enable dynamic data discovery

# Configuration Paths
SECTOR_CONFIG_PATH=config/sector_models.toml
AUTONOMOUS_TRAINING_CONFIG=config/autonomous_training.toml
DATA_REQUIREMENTS_CONFIG=config/data_requirements.toml

# Runtime Overrides
RUST_LOG=info                       # Logging level
DATABASE_URL=postgres://...         # Override database connection
REDIS_URL=redis://...              # Override Redis connection
```

## Configuration Loading Order

1. **Base Configuration**: `config/platform.toml` loaded first
2. **Environment Override**: `config/{environment}.toml` merged on top
3. **External Configs**: Referenced files loaded (sector_models.toml, etc.)
4. **Environment Variables**: Override any setting
5. **Runtime Flags**: Command-line arguments take precedence

## Settings Reference

### Performance Impact Settings

| Setting | Impact | Default | Recommendation |
|---------|--------|---------|----------------|
| `memory_target_mb_per_symbol` | Memory usage | 50 | Keep at 50 for Phase 2 compliance |
| `shared_feature_extraction` | Memory efficiency | true | Always enable for 90% reduction |
| `gradient_update_latency_ms` | Real-time performance | 50 | Lower for faster adaptation |
| `max_concurrent_predictions` | Throughput | 8 | Increase with more CPU cores |

### Critical Thresholds

| Threshold | Purpose | Default | Notes |
|-----------|---------|---------|-------|
| `accuracy_threshold` | Training trigger | 0.8 | Below this triggers retraining |
| `consensus_threshold` | DAA voting | 0.7 | 70% agreement required |
| `memory_pressure_threshold` | Resource management | 0.85 | Deactivate models above this |
| `data_quality_threshold` | Data acceptance | 0.7 | Reject data below this quality |

### Feature Control Settings

| Setting | Feature | Impact |
|---------|---------|--------|
| `enable_sector_coordinators` | Hierarchical DAA | Enables multi-level trading decisions |
| `dynamic_type_registration` | Data discovery | Allows runtime data type addition |
| `market_aware_scheduling` | Training timing | Respects market hours for training |
| `adaptive_learning_rates` | Real-time training | Adjusts learning based on market |

## Best Practices

1. **Memory Configuration**
   - Set `memory_gb` based on available system RAM
   - Enable `shared_feature_extraction` for efficiency
   - Use `lazy_loading_enabled` for large deployments

2. **Performance Tuning**
   - Adjust `max_concurrent_predictions` based on CPU
   - Lower `gradient_update_latency_ms` for faster adaptation
   - Set appropriate `checkpoint_interval_minutes`

3. **Risk Management**
   - Never lower `consensus_threshold` below 0.7
   - Keep `neural_weight` at 0.6 for balanced decisions
   - Set conservative `max_positions` limits

4. **Data Quality**
   - Set `quality_threshold` based on data source reliability
   - Enable `outlier_detection_enabled` for noisy data
   - Configure appropriate `stale_data_threshold_hours`

5. **Production Deployment**
   - Always use environment-specific override files
   - Set conservative resource limits initially
   - Enable monitoring and alerting
   - Test configuration changes in staging first

## Troubleshooting

### Common Issues

1. **Models not activating**
   - Check `data_requirements.toml` characteristic matching
   - Verify data quality meets thresholds
   - Ensure `auto_model_activation = true`

2. **High memory usage**
   - Enable `shared_feature_extraction`
   - Reduce `max_active_models_per_symbol`
   - Lower `feature_cache_ttl_seconds`

3. **Poor prediction performance**
   - Check `accuracy_threshold` settings
   - Verify `model_weights` are appropriate
   - Ensure data quality meets requirements

4. **Training not triggering**
   - Verify `ENABLE_AUTONOMOUS_TRAINING=true`
   - Check performance thresholds
   - Ensure sufficient data samples available

## Related Documentation

- **[SECTOR_ETF_REFERENCE.md](SECTOR_ETF_REFERENCE.md)**: Complete guide to sector ETF mappings and usage
- **[HIGH_LEVEL_FEATURE_PLAN.md](HIGH_LEVEL_FEATURE_PLAN.md)**: Overall transformation plan
- **[phase2/plan/](phase2/plan/)**: Phase 2 sector architecture documentation
- **[phase3/plan/](phase3/plan/)**: Phase 3 autonomous training documentation

## Version History

- **v2.0.0**: Added sector models and hierarchical DAA (Phase 2)
- **v3.0.0**: Added autonomous training and data discovery (Phase 3)
- **v3.1.0**: Real-time adaptive training capabilities

---

For additional configuration support, see the technical documentation in `products/features/nrevamp/` directory.