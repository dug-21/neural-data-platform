# Docker Volume Configuration for Model Persistence

This directory contains configuration files for Docker volume persistence, specifically for neural network models.

## Files

### models.yml

Configuration file for model storage settings mounted at `/app/config/models.yml` in the container.

Key settings:
- **base_path**: `/app/models` - Where models are stored in the container
- **max_checkpoints_per_model**: 10 - Number of checkpoints to retain
- **archive_retention_days**: 90 - How long to keep archived models
- **storage_quota_mb**: 5000 - Storage limit per model type

## Volume Mount Points

The production Docker Compose configuration mounts:
1. `neural_trader_models:/app/models` - Named volume for model persistence
2. `./docker/production/volumes/models.yml:/app/config/models.yml:ro` - Configuration file (read-only)

## Model Directory Structure

```
/app/models/
├── checkpoints/     # Training checkpoints
│   ├── NHITS/
│   ├── TCN/
│   ├── DeepAR/
│   └── MLP/
├── production/      # Production-ready models
│   ├── NHITS/
│   └── MLP/
├── archive/         # Compressed archived models
└── backups/        # Model backups
```

## Health Checks

The system monitors:
- Model availability (count and types)
- Required models presence (NHITS, MLP)
- Volume mount status
- Storage usage

## Initialization

On container startup:
1. `/app/scripts/init-models.sh` creates the directory structure
2. Sets proper permissions for the neuraltrader user
3. Creates `.initialized` marker file
4. Checks for existing models

## Monitoring

Prometheus metrics track:
- `neural_trader_models_available` - Number of available models
- `neural_trader_required_models_missing` - Count of missing required models
- `neural_trader_model_storage_mounted` - Volume mount status

## Usage

Deploy with:
```bash
docker-compose -f docker-compose.prod.yml up -d
```

The models will persist across container restarts and updates.