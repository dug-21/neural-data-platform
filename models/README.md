# Neural Trader Models Directory

This directory contains trained ruv-fann neural network models for the Neural Trader system.

## Quick Start

### Loading a Model
```rust
use neural_trader::models::ModelLoader;

// Load the current production model for AAPL
let model = ModelLoader::load_current("AAPL", "prediction")?;

// Load a specific version
let model = ModelLoader::load_version("AAPL", "prediction", "1.0.0")?;
```

### Model Types

- **prediction**: Price prediction models (primary)
- **classification**: Market direction classifiers
- **regression**: Continuous value predictors
- **ensemble**: Combined model ensembles

## Directory Structure

```
models/
├── {SYMBOL}/              # Per-symbol models (e.g., AAPL, MSFT)
│   ├── current -> ...     # Symlink to current production
│   ├── previous -> ...    # Symlink to previous version
│   └── {model_type}/      # Model type directories
│       └── v{x.y.z}/      # Versioned model storage
└── shared/                # Shared components
```

## Model Files

Each model version directory contains:
- `model.rvf` - The ruv-fann model binary
- `metadata.json` - Model metadata and metrics
- `config.json` - Training configuration
- `performance.json` - Performance benchmarks
- `features.json` - Feature definitions

## Docker Volume Mounting

In production, models are mounted read-only:
```yaml
volumes:
  - ./models:/app/models:ro
```

## Model Management Commands

```bash
# List all models
./scripts/list_models.sh

# Validate a model
./scripts/validate_model.sh AAPL prediction v1.0.0

# Promote model to production
./scripts/promote_model.sh AAPL prediction v1.1.0

# Rollback to previous version
./scripts/rollback_model.sh AAPL prediction
```

## Best Practices

1. **Never modify** production model files directly
2. **Always validate** new models before promotion
3. **Document changes** in model metadata
4. **Test thoroughly** in staging environment
5. **Monitor performance** after deployment

## See Also

- [STORAGE_ARCHITECTURE.md](./STORAGE_ARCHITECTURE.md) - Detailed architecture documentation
- [Training Guide](../docs/TRAINING.md) - Model training procedures
- [Deployment Guide](../docs/DEPLOYMENT.md) - Production deployment