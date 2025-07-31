# Neural Trader Model Storage Architecture

## Overview

This document describes the model storage architecture for the Neural Trader system, designed to support ruv-fann neural models with versioning, metadata management, and Docker persistence.

## Directory Structure

```
models/
├── .gitignore                    # Excludes model binaries from git
├── .metadata/                    # Global metadata and indices
│   ├── model_registry.json       # Registry of all models
│   ├── performance_metrics.json  # Historical performance tracking
│   └── deployment_history.json   # Deployment and rollback history
├── .templates/                   # Templates for metadata files
│   ├── model_metadata_template.json
│   └── version_manifest_template.json
├── .backups/                     # Model backup storage
│   └── {timestamp}/              # Timestamped backup folders
├── README.md                     # Usage documentation
├── {symbol}/                     # Per-symbol model storage (e.g., AAPL, MSFT)
│   ├── current -> {model_type}/v{x.y.z}  # Symlink to current production model
│   ├── previous -> {model_type}/v{x.y.z} # Symlink to previous version
│   ├── {model_type}/             # Model type directory
│   │   ├── latest -> v{x.y.z}    # Symlink to latest version
│   │   └── v{major}.{minor}.{patch}/     # Version directory
│   │       ├── model.rvf         # Ruv-fann model file
│   │       ├── model.rvf.sig     # Model signature for integrity
│   │       ├── metadata.json     # Model metadata
│   │       ├── config.json       # Training configuration
│   │       ├── performance.json  # Performance metrics
│   │       ├── features.json     # Feature definitions
│   │       ├── checkpoints/      # Training checkpoints
│   │       │   └── checkpoint_{epoch}.rvf
│   │       └── artifacts/        # Additional artifacts
│   │           ├── training_log.jsonl
│   │           ├── validation_curves.png
│   │           └── feature_importance.json
│   └── ensemble/                 # Ensemble model combinations
│       └── v{x.y.z}/
│           ├── ensemble_config.json
│           └── component_models.json
└── shared/                       # Shared model components
    ├── feature_extractors/       # Reusable feature extraction models
    ├── preprocessors/            # Data preprocessing models
    └── postprocessors/           # Output processing models
```

## Model Types

1. **prediction**: Main price prediction models
2. **classification**: Market direction classification models
3. **regression**: Continuous value regression models
4. **ensemble**: Combined model ensembles

## Versioning Schema

Models follow semantic versioning (SemVer):
- **Major (X.0.0)**: Breaking changes, incompatible model architecture
- **Minor (0.Y.0)**: New features, backward compatible improvements
- **Patch (0.0.Z)**: Bug fixes, performance improvements

## File Formats

### Model Files (.rvf)
- **Format**: Ruv-FANN proprietary format
- **Content**: Neural network weights, architecture, activation functions
- **Compression**: Optional ZSTD compression for large models

### Metadata Files (metadata.json)
```json
{
  "model_id": "uuid-v4",
  "symbol": "AAPL",
  "model_type": "prediction",
  "version": "1.0.0",
  "created_at": "2025-01-29T12:00:00Z",
  "trained_by": "system|user_id",
  "training_duration_seconds": 3600,
  "architecture": {
    "input_size": 50,
    "hidden_layers": [128, 64, 32],
    "output_size": 1,
    "activation": "relu",
    "optimizer": "adam"
  },
  "training_data": {
    "start_date": "2020-01-01",
    "end_date": "2024-12-31",
    "total_samples": 1000000,
    "features": ["price", "volume", "indicators"]
  },
  "performance": {
    "training_loss": 0.0023,
    "validation_loss": 0.0031,
    "test_metrics": {
      "mse": 0.0035,
      "mae": 0.0021,
      "r2": 0.94
    }
  },
  "deployment": {
    "status": "production|staging|archived",
    "deployed_at": "2025-01-29T13:00:00Z",
    "served_requests": 0
  }
}
```

### Configuration Files (config.json)
```json
{
  "training_config": {
    "batch_size": 32,
    "learning_rate": 0.001,
    "epochs": 100,
    "early_stopping": {
      "patience": 10,
      "min_delta": 0.0001
    }
  },
  "feature_engineering": {
    "window_size": 50,
    "indicators": ["sma", "ema", "rsi", "macd"],
    "normalization": "minmax"
  },
  "validation": {
    "split_ratio": 0.2,
    "cross_validation_folds": 5
  }
}
```

## Docker Volume Integration

### Production Docker Compose Addition
```yaml
volumes:
  neural_models:
    driver: local
    driver_opts:
      type: none
      o: bind
      device: ./models

services:
  neural-trader:
    volumes:
      - neural_models:/app/models:ro  # Read-only in production
      - neural_models_cache:/app/models/.cache:rw  # Writable cache
```

### Model Loading in Application
```yaml
environment:
  - MODEL_STORAGE_PATH=/app/models
  - MODEL_CACHE_PATH=/app/models/.cache
  - MODEL_HOT_RELOAD=false  # True in development
```

## Deployment Workflow

### 1. Model Training
```bash
# Train new model version
./scripts/train_model.sh --symbol AAPL --type prediction --version 1.1.0
```

### 2. Model Validation
```bash
# Validate model performance
./scripts/validate_model.sh --path models/AAPL/prediction/v1.1.0
```

### 3. Model Promotion
```bash
# Promote to production
./scripts/promote_model.sh --symbol AAPL --type prediction --version 1.1.0
```

### 4. Rollback Capability
```bash
# Rollback to previous version
./scripts/rollback_model.sh --symbol AAPL --type prediction
```

## Backup Strategy

### Automated Backups
- **Frequency**: Daily at 02:00 UTC
- **Retention**: 30 days for daily, 12 months for monthly
- **Location**: `.backups/{timestamp}/`
- **Format**: Compressed tar archives with checksums

### Manual Backup
```bash
./scripts/backup_models.sh --symbols all --compress true
```

## Access Control

### File Permissions
```bash
models/
├── [drwxr-xr-x] .metadata/      # Read/write for system
├── [drwxr-xr-x] {symbol}/       # Read/write for training
│   └── [dr-xr-xr-x] */current/  # Read-only in production
└── [drwxr-xr-x] .backups/       # Write-only for backup service
```

### Docker Container Access
- **Production**: Read-only mount for model serving
- **Training**: Read/write mount for model updates
- **Backup**: Write-only mount for backup service

## Monitoring and Alerting

### Model Performance Tracking
- Track inference latency per model version
- Monitor prediction accuracy drift
- Alert on model degradation

### Storage Metrics
- Disk usage per symbol and model type
- Model file integrity checks
- Version proliferation warnings

## Migration Guide

### From Legacy Storage
```bash
# Migrate existing models
./scripts/migrate_models.sh --from /old/models --to /app/models
```

### Version Upgrade
```bash
# Upgrade model format
./scripts/upgrade_model_format.sh --path models/AAPL/prediction/v1.0.0
```

## Best Practices

1. **Version Control**: Never modify existing version directories
2. **Testing**: Always validate new models before promotion
3. **Documentation**: Update metadata for every model change
4. **Cleanup**: Archive old versions based on retention policy
5. **Security**: Sign models for integrity verification

## Disaster Recovery

### Recovery Procedure
1. Stop application services
2. Restore from latest backup
3. Verify model integrity
4. Update symlinks
5. Restart services

### Health Checks
```bash
# Verify all models
./scripts/verify_models.sh --check-integrity --check-performance
```

## Future Enhancements

1. **Distributed Storage**: Support for S3/MinIO object storage
2. **Model Registry**: Central model registry service
3. **A/B Testing**: Support for gradual model rollouts
4. **Federated Learning**: Multi-site model aggregation
5. **Model Compression**: Automatic model optimization for edge deployment