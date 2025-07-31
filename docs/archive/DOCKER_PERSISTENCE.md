# Docker Model Persistence Strategy

## Overview

This document describes the model persistence strategy for the Neural Trader Docker production deployment. The system ensures that trained neural network models persist across container restarts and deployments.

## Architecture

### Volume Mounting Strategy

The production deployment uses Docker named volumes to persist model data:

```yaml
volumes:
  neural_trader_models:  # Named volume for model persistence
```

This volume is mounted to the neural-trader service at `/app/models`:

```yaml
neural-trader:
  volumes:
    - neural_trader_models:/app/models
    - ./docker/production/volumes/models.yml:/app/config/models.yml:ro
```

### Directory Structure

The model storage follows this hierarchy:

```
/app/models/
├── checkpoints/     # Training checkpoints and intermediate models
│   ├── NHITS/      # NHITS model checkpoints
│   ├── TCN/        # TCN model checkpoints
│   ├── DeepAR/     # DeepAR model checkpoints
│   └── MLP/        # MLP model checkpoints
├── production/      # Production-ready models
│   ├── NHITS/      # Active NHITS models
│   └── MLP/        # Active MLP models
├── archive/         # Archived models (compressed)
└── backups/        # Model backups
```

## Configuration

### Model Storage Configuration (models.yml)

The `docker/production/volumes/models.yml` file provides configuration for model storage:

```yaml
model_storage:
  base_path: /app/models
  max_checkpoints_per_model: 10
  archive_retention_days: 90
  enable_compression: true
  storage_quota_mb: 5000
```

### Environment Variables

The neural-trader service includes the `MODEL_STORAGE_PATH` environment variable:

```yaml
environment:
  - MODEL_STORAGE_PATH=/app/models
```

## Health Checks

### Model Availability Monitoring

The health check system verifies model availability:

1. **Directory Check**: Verifies `/app/models` exists and is accessible
2. **Model Count**: Counts available models in checkpoints and production directories
3. **Required Models**: Ensures required models (NHITS, MLP) are present
4. **Health Status**:
   - **Healthy**: All required models available
   - **Degraded**: Some required models missing
   - **Unhealthy**: No models available

### Health Check Endpoint

The `/health` endpoint includes model status:

```json
{
  "components": {
    "NeuralSystem": {
      "status": "Healthy",
      "metadata": {
        "model_count": "4",
        "available_models": "production/NHITS, production/MLP",
        "required_models": "NHITS, MLP"
      }
    }
  }
}
```

## Deployment Process

### Initial Deployment

1. **Volume Creation**: Docker creates the `neural_trader_models` volume on first deployment
2. **Directory Initialization**: The Dockerfile creates `/app/models` with proper permissions
3. **Model Loading**: Application loads pre-trained models or trains new ones

### Updates and Redeployments

1. **Volume Persistence**: The named volume persists across container updates
2. **Zero Downtime**: New containers connect to existing volume
3. **Model Continuity**: All trained models remain available

### Backup Strategy

The backup service includes model backups:

```yaml
backup:
  volumes:
    - neural_trader_models:/models:ro
    - backup_prod_data:/backups
```

## Best Practices

### 1. Pre-deployment Checks

Before deploying, ensure:
- Models are trained and saved to the volume
- Health checks pass for required models
- Backup of current models exists

### 2. Model Versioning

Each model save includes:
- Timestamp-based versioning
- Metadata with training info
- Performance metrics

### 3. Storage Management

The system automatically:
- Removes old checkpoints beyond retention limit
- Archives models older than 30 days
- Compresses archived models to save space

### 4. Monitoring

Monitor:
- Model storage usage via Prometheus metrics
- Model availability via health checks
- Model performance via Grafana dashboards

## Troubleshooting

### Models Not Persisting

1. Check volume mount: `docker inspect neural-trader_neural_trader_models`
2. Verify permissions: Container user must have write access
3. Check disk space: Ensure sufficient space on Docker host

### Health Check Failures

1. Verify model directory: `docker exec neural-trader ls -la /app/models`
2. Check model metadata: Ensure metadata.json files exist
3. Review logs: `docker logs neural-trader | grep model`

### Recovery Procedures

1. **From Backup**:
   ```bash
   docker run --rm -v neural_trader_models:/models \
     -v backup_prod_data:/backups:ro \
     alpine sh -c "cp -r /backups/models/* /models/"
   ```

2. **From Archive**:
   ```bash
   docker exec neural-trader \
     /app/neural-trader restore-models --from-archive
   ```

## Docker Compose Commands

### Deploy with Model Persistence

```bash
# Initial deployment
docker-compose -f docker-compose.prod.yml up -d

# Update deployment (preserves volumes)
docker-compose -f docker-compose.prod.yml up -d --no-recreate

# Scale with shared models
docker-compose -f docker-compose.prod.yml up -d --scale neural-trader=3
```

### Volume Management

```bash
# List volumes
docker volume ls | grep neural_trader

# Inspect model volume
docker volume inspect neural-trader_neural_trader_models

# Backup volume
docker run --rm -v neural-trader_neural_trader_models:/source \
  -v $(pwd)/backup:/backup alpine tar -czf /backup/models.tar.gz -C /source .
```

## Integration with CI/CD

### GitLab CI Example

```yaml
deploy:
  stage: deploy
  script:
    - docker-compose -f docker-compose.prod.yml pull
    - docker-compose -f docker-compose.prod.yml up -d --no-recreate
    - docker-compose -f docker-compose.prod.yml exec -T neural-trader \
        curl -f http://localhost:3030/health || exit 1
```

### Health Check Validation

```bash
# Verify models after deployment
curl -s http://localhost:3030/health | jq '.components.NeuralSystem'
```

## Security Considerations

1. **Volume Encryption**: Use Docker volume encryption for sensitive model data
2. **Access Control**: Limit volume access to neural-trader service only
3. **Backup Encryption**: Encrypt model backups at rest
4. **Network Isolation**: Models accessible only within backend network

## Performance Optimization

1. **SSD Storage**: Use SSD-backed storage for model volumes
2. **Memory Caching**: Frequently used models cached in memory
3. **Lazy Loading**: Models loaded on-demand to reduce startup time
4. **Compression**: Archived models compressed to save space

## Conclusion

This persistence strategy ensures:
- ✅ Models survive container restarts
- ✅ Zero-downtime deployments
- ✅ Automatic health monitoring
- ✅ Backup and recovery capabilities
- ✅ Scalable architecture

The combination of Docker named volumes, health checks, and proper configuration provides a robust model persistence solution for production deployments.