# Neural Model Rollback System

## Overview

The Model Rollback System provides production-grade safety mechanisms for neural model deployments with atomic updates, automatic performance monitoring, and instant rollback capabilities.

## Key Features

### 1. **Atomic Model Updates**
- Symlink-based deployment strategy ensures zero-downtime updates
- Current model always accessible via `/models/{model_name}/current`
- Atomic rename operations prevent partial updates
- Docker container restart safety through persistent symlinks

### 2. **Automatic Performance Monitoring**
- Continuous health checks after deployment
- Performance degradation detection
- Configurable thresholds and evaluation periods
- Grace period for model warm-up

### 3. **Automatic Rollback**
- Triggers on performance degradation (>10% by default)
- Monitors accuracy, latency, error rate
- Reverts to previous known-good version
- Preserves rollback history for auditing

### 4. **Manual Rollback Tools**
- CLI tool for operator intervention
- Rollback with reason tracking
- Version history inspection
- Integrity verification

### 5. **Version Management**
- Configurable version retention (default: 5)
- Automatic archival of old versions
- Metadata backup for disaster recovery
- SHA256 checksums for integrity

## Architecture

```
/opt/neural-trader/models/
├── enhanced_mlp/
│   ├── current -> enhanced_mlp-1705339200000/
│   ├── enhanced_mlp-1705339200000/
│   │   ├── model.bin
│   │   └── config.json
│   ├── enhanced_mlp-1705252800000/
│   │   ├── model.bin
│   │   └── config.json
│   └── archive/
│       └── enhanced_mlp-1705166400000/
└── metadata/
    ├── enhanced_mlp-1705339200000.json
    └── enhanced_mlp-1705252800000.json
```

## Configuration

```toml
[rollback]
model_base_dir = "/opt/neural-trader/models"
metadata_backup_path = "/opt/neural-trader/metadata"
max_versions = 5
degradation_threshold = 10.0  # Percentage
evaluation_period = 300       # Seconds
sample_count = 20
enable_auto_rollback = true
health_check_interval = 30    # Seconds
grace_period = 60            # Seconds
enable_metadata_backup = true
```

## CLI Usage

### List Model Versions
```bash
model-rollback list enhanced_mlp
```

### Show Current Version
```bash
model-rollback current enhanced_mlp
```

### Manual Rollback
```bash
model-rollback rollback enhanced_mlp \
  --reason "High error rate in production" \
  --user "admin"
```

### View Rollback History
```bash
model-rollback history enhanced_mlp
```

### Verify Model Integrity
```bash
model-rollback verify enhanced_mlp
```

### Cleanup Old Archives
```bash
model-rollback cleanup enhanced_mlp --keep 3
```

## Integration Example

```rust
use autonomous_platform::adapters::model_rollback::{
    ModelRollbackManager, RollbackConfig, ModelMetrics
};

// Configure rollback manager
let config = RollbackConfig::default();
let manager = ModelRollbackManager::new(config)?;

// Deploy new model
let metrics = ModelMetrics {
    accuracy: 95.0,
    latency_ms: 50.0,
    error_rate: 5.0,
    memory_mb: 150,
    cpu_percent: 30.0,
    throughput: 20.0,
    timestamp: Utc::now(),
};

let version = manager.deploy_model(
    "enhanced_mlp",
    &model_path,
    config_json,
    metrics,
).await?;

// Automatic monitoring starts immediately
// Rollback triggered if performance degrades
```

## Docker Integration

### Container Restart Safety
The symlink strategy ensures models remain accessible after container restarts:

1. Models stored in persistent volume
2. Symlinks recreated on startup if needed
3. Metadata backup for recovery

### Docker Compose Example
```yaml
services:
  neural-trader:
    image: neural-trader:latest
    volumes:
      - models:/opt/neural-trader/models
      - metadata:/opt/neural-trader/metadata
    environment:
      - MODEL_BASE_DIR=/opt/neural-trader/models
      - ENABLE_AUTO_ROLLBACK=true

volumes:
  models:
  metadata:
```

## Performance Monitoring

### Tracked Metrics
- **Accuracy**: Model prediction accuracy
- **Latency**: Average prediction time
- **Error Rate**: Percentage of failed predictions
- **Memory Usage**: RAM consumption
- **CPU Usage**: Processor utilization
- **Throughput**: Predictions per second

### Degradation Detection
Automatic rollback triggers when:
- Accuracy drops >10% from baseline
- Latency increases >10% from baseline
- Error rate increases >10% from baseline

### Monitoring Flow
```
Deploy Model → Grace Period → Start Monitoring → Collect Samples
     ↓                                                    ↓
     ↓                                          Check Degradation
     ↓                                                    ↓
     ↓                                            If Degraded
     ↓                                                    ↓
     └←←←←←←←←← Rollback to Previous ←←←←←←←←←←←←←←←←←←┘
```

## Best Practices

### 1. **Pre-deployment Testing**
- Test models thoroughly before production deployment
- Establish baseline metrics in staging environment
- Validate model compatibility

### 2. **Configuration Tuning**
- Adjust `degradation_threshold` based on model sensitivity
- Set appropriate `grace_period` for model warm-up
- Configure `evaluation_period` for stable metrics

### 3. **Monitoring Integration**
- Connect with existing monitoring systems
- Set up alerts for rollback events
- Track rollback frequency as quality metric

### 4. **Version Management**
- Keep at least 3 previous versions
- Regular cleanup of archives
- Backup metadata to separate storage

### 5. **Manual Intervention**
- Document rollback procedures
- Train operators on CLI usage
- Maintain rollback reason log

## Troubleshooting

### Model Not Found
```bash
# Check symlink status
ls -la /opt/neural-trader/models/enhanced_mlp/current

# Verify model integrity
model-rollback verify enhanced_mlp
```

### Rollback Fails
```bash
# Check version history
model-rollback list enhanced_mlp

# Manual symlink update (emergency)
ln -sfn /path/to/version /path/to/current
```

### Performance Monitoring Issues
```bash
# Check health monitor logs
journalctl -u neural-trader | grep "health_monitor"

# Disable auto-rollback temporarily
export ENABLE_AUTO_ROLLBACK=false
```

## Security Considerations

1. **File Permissions**: Ensure model directories have appropriate permissions
2. **Checksum Verification**: Always verify model integrity after deployment
3. **Audit Trail**: Maintain logs of all deployments and rollbacks
4. **Access Control**: Restrict CLI tool access to authorized operators

## Future Enhancements

- [ ] Multi-region rollback coordination
- [ ] A/B testing support with gradual rollout
- [ ] Integration with CI/CD pipelines
- [ ] Automated rollback testing
- [ ] Machine learning for optimal thresholds