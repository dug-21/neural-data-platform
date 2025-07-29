# Model Storage Architecture

## Overview

This document defines the production model storage architecture for the Neural Trader system. The architecture supports versioned ruv-fann models with proper metadata, rollback capabilities, and Docker deployment compatibility.

## Directory Structure

```
models/
├── {symbol}/                    # Trading symbol (AAPL, GOOGL, MSFT, TSLA)
│   ├── prediction/             # Price prediction models
│   │   ├── v{major}.{minor}.{patch}/  # Semantic versioning
│   │   │   ├── model.fann      # FANN neural network file
│   │   │   ├── weights.bin     # Binary weights (optional)
│   │   │   ├── config.json     # Model-specific configuration
│   │   │   └── validation/     # Validation results
│   │   ├── current -> v1.0.0   # Symlink to active version
│   │   ├── metadata/           # Model metadata and performance
│   │   └── backups/           # Automated backups
│   ├── momentum/              # Momentum strategy models
│   └── reversal/              # Mean reversion models
├── templates/                 # Base model templates
│   ├── prediction/
│   ├── momentum/
│   └── reversal/
├── shared/                   # Shared resources
│   ├── common/              # Common utilities
│   ├── utils/               # Helper functions
│   └── configs/             # Default configurations
└── archive/                 # Historical models
    ├── ensemble/
    ├── lstm/
    ├── gru/
    ├── mlp/
    └── transformer/
```

## Versioning Strategy

### Semantic Versioning (SemVer)

Models follow semantic versioning: `v{major}.{minor}.{patch}`

- **Major**: Breaking changes to model architecture or input/output format
- **Minor**: Backward-compatible improvements (new features, better training)
- **Patch**: Bug fixes, parameter tuning, minor improvements

### Version Management

```bash
# Example version progression
v1.0.0  # Initial production model
v1.0.1  # Bug fix in preprocessing
v1.1.0  # Added new features to input
v2.0.0  # Changed architecture from 3-layer to 5-layer
```

### Current Version Symlinks

Each model type maintains a `current` symlink pointing to the active production version:

```bash
models/AAPL/prediction/current -> v1.0.0
models/AAPL/momentum/current -> v1.2.3
models/AAPL/reversal/current -> v2.1.0
```

## File Formats and Structure

### Model Files

#### Primary Model File: `model.fann`
- FANN format neural network
- Contains architecture and trained weights
- Directly loadable by ruv-fann library

#### Configuration File: `config.json`
```json
{
  "model_name": "AAPL_prediction_v1.0.0",
  "symbol": "AAPL",
  "model_type": "prediction",
  "version": "1.0.0",
  "created_at": "2025-07-29T01:00:00Z",
  "framework": "ruv-fann",
  "architecture": {
    "type": "feedforward",
    "layers": 3,
    "input_neurons": 10,
    "hidden_neurons": [20],
    "output_neurons": 1,
    "activation_function": "sigmoid"
  },
  "training_config": {
    "learning_rate": 0.7,
    "epochs": 1000,
    "training_algorithm": "RPROP",
    "desired_error": 0.001
  },
  "performance_metrics": {
    "train_mse": 0.0023,
    "validation_mse": 0.0031,
    "test_accuracy": 0.847,
    "sharpe_ratio": 1.23
  },
  "features": [...],
  "deployment_status": "production",
  "checksum": "sha256:abcd1234ef567890..."
}
```

#### Metadata Directory
- `model_info.json`: Comprehensive model information
- `performance_history.json`: Historical performance metrics
- `deployment_log.json`: Deployment and rollback history

### Validation Results

Each model version includes validation results:

```
v1.0.0/validation/
├── backtest_results.json       # Backtesting performance
├── cross_validation.json       # K-fold validation results
├── feature_importance.json     # Feature analysis
└── error_analysis.json        # Error distribution analysis
```

## Model Types and Specialization

### Prediction Models
- **Purpose**: Price direction and magnitude prediction
- **Input Features**: 10 technical indicators
- **Output**: Single continuous value (price change %)
- **Architecture**: 3-layer feedforward (10-20-1)

### Momentum Models  
- **Purpose**: Trend continuation signals
- **Input Features**: 15 momentum indicators
- **Output**: 3 classes (strong_up, weak, strong_down)
- **Architecture**: 3-layer feedforward (15-30-3)

### Reversal Models
- **Purpose**: Mean reversion opportunities
- **Input Features**: 12 reversal indicators  
- **Output**: 2 classes (reversal, continuation)
- **Architecture**: 3-layer feedforward (12-25-2)

## Deployment Integration

### Docker Volume Mounting

```yaml
# docker-compose.yml
services:
  neural-trader:
    volumes:
      - ./models:/app/models:ro  # Read-only model access
      - model_cache:/app/cache   # Writable cache volume
```

### Model Loading Strategy

1. **Startup**: Load all `current` models into memory
2. **Runtime**: Cache frequently accessed models
3. **Updates**: Hot-swap models without service restart
4. **Fallback**: Automatic rollback on performance degradation

### Environment Integration

```rust
// Model loading in Rust
use ruv_fann::Fann;

pub fn load_production_model(symbol: &str, model_type: &str) -> Result<Fann, Error> {
    let model_path = format!("models/{}/{}/current/model.fann", symbol, model_type);
    Fann::new(&model_path)
}
```

## Backup and Recovery

### Automated Backup Strategy

- **Daily**: Copy current models to timestamped backup
- **Weekly**: Archive old versions to reduce disk usage
- **Pre-deployment**: Automatic backup before model updates

### Backup Directory Structure

```
models/{symbol}/{model_type}/backups/
├── 2025-07-29_daily_backup.tar.gz
├── 2025-07-28_daily_backup.tar.gz
├── 2025-07-22_weekly_backup.tar.gz
└── pre_v1.1.0_deployment.tar.gz
```

### Recovery Procedures

1. **Model Corruption**: Restore from daily backup
2. **Performance Degradation**: Rollback to previous version
3. **Architecture Changes**: Restore from pre-deployment backup

## Security and Access Control

### File Permissions

```bash
# Production deployment permissions
models/              # 755 (rwxr-xr-x)
├── symbol/          # 755 (rwxr-xr-x)  
│   ├── model_type/  # 755 (rwxr-xr-x)
│   │   ├── version/ # 755 (rwxr-xr-x)
│   │   │   ├── model.fann    # 644 (rw-r--r--)
│   │   │   └── config.json   # 644 (rw-r--r--)
│   │   └── current  # 777 (rwxrwxrwx) - symlink
```

### Access Patterns

- **Read-Only**: Production model loading
- **Write Access**: Model training and deployment processes
- **Admin Access**: Backup and recovery operations

## Monitoring and Observability

### Model Health Checks

```json
{
  "model_health": {
    "symbol": "AAPL",
    "model_type": "prediction", 
    "version": "v1.0.0",
    "status": "healthy",
    "last_prediction": "2025-07-29T01:30:00Z",
    "prediction_latency_ms": 12,
    "error_rate": 0.002,
    "performance_drift": 0.15
  }
}
```

### Metrics Collection

- **Prediction Latency**: Time to generate predictions
- **Model Accuracy**: Real-time accuracy tracking
- **Memory Usage**: Model memory footprint
- **Disk Usage**: Storage utilization monitoring

## Integration with Trading System

### Model Registry

```rust
pub struct ModelRegistry {
    pub prediction_models: HashMap<String, Fann>,  // symbol -> model
    pub momentum_models: HashMap<String, Fann>,
    pub reversal_models: HashMap<String, Fann>,
    pub metadata_cache: HashMap<String, ModelMetadata>,
}
```

### Feature Pipeline Integration

```rust
// Example feature extraction for model input
pub fn prepare_model_input(
    market_data: &MarketData,
    symbol: &str
) -> Result<Vec<f32>, Error> {
    let features = vec![
        market_data.price_change_5m(),
        market_data.volume_ratio(),
        market_data.rsi_14(),
        // ... other features
    ];
    Ok(features)
}
```

## Performance Optimization

### Caching Strategy

1. **Model Caching**: Keep frequently used models in memory
2. **Prediction Caching**: Cache recent predictions with TTL
3. **Feature Caching**: Cache computed features across models

### Resource Management

- **Memory Limits**: Monitor model memory usage
- **Disk Cleanup**: Automated cleanup of old versions
- **CPU Optimization**: Parallel model loading and inference

## Migration and Upgrades

### Model Update Process

1. **Training**: Train new model version
2. **Validation**: Comprehensive testing and validation
3. **Staging**: Deploy to staging environment
4. **A/B Testing**: Compare against current production model
5. **Deployment**: Update symlink to new version
6. **Monitoring**: Monitor performance post-deployment
7. **Rollback**: Automatic rollback if performance degrades

### Version Compatibility

- **Forward Compatibility**: New versions handle old data formats
- **Backward Compatibility**: Maintain API consistency
- **Migration Scripts**: Automated data format migrations

## Troubleshooting

### Common Issues

1. **Model Loading Failures**
   - Check file permissions
   - Verify model file integrity
   - Validate symlink targets

2. **Performance Degradation**
   - Monitor prediction accuracy
   - Check for data drift
   - Validate feature consistency

3. **Disk Space Issues**
   - Run cleanup scripts
   - Archive old versions
   - Monitor disk usage alerts

### Debugging Tools

```bash
# Model validation script
./scripts/validate_model.sh AAPL prediction v1.0.0

# Performance benchmark
./scripts/benchmark_model.sh AAPL prediction

# Disk usage analysis
./scripts/analyze_model_storage.sh
```

## Future Enhancements

### Planned Features

1. **Model Ensemble**: Combine multiple model versions
2. **Auto-scaling**: Dynamic model loading based on demand
3. **Distributed Storage**: Multi-node model distribution
4. **Model Compression**: Reduce model size for faster loading
5. **Real-time Retraining**: Continuous model improvement

### Research Areas

- **Neural Architecture Search**: Automated architecture optimization
- **Transfer Learning**: Share knowledge between symbol models
- **Federated Learning**: Distributed training across nodes
- **Model Interpretability**: Better understanding of model decisions

---

This architecture provides a robust, scalable foundation for managing ruv-fann neural network models in production trading environments, with emphasis on reliability, performance, and operational excellence.