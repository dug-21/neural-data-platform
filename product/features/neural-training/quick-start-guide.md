# Autonomous Model Training Quick Start Guide

## 5-Minute Setup

Get autonomous model training running in your neural-trader deployment in just 5 minutes.

### Prerequisites

- Neural-trader system running with DAA components
- Python 3.8+ and Rust 1.70+
- Docker and docker-compose
- 16GB+ RAM, 50GB+ disk space
- GPU recommended for faster training

### Quick Installation

#### 1. Enable Autonomous Training (30 seconds)

```bash
# Add to your .env file
echo "AUTONOMOUS_TRAINING_ENABLED=true" >> .env
echo "TRAINING_EVALUATION_INTERVAL=300" >> .env
echo "TRAINING_MAX_CONCURRENT_JOBS=2" >> .env
```

#### 2. Deploy Training Coordinator (1 minute)

```bash
# Pull and start the training coordinator
docker-compose up -d training-coordinator

# Verify it's running
docker-compose ps training-coordinator
```

#### 3. Configure Basic Settings (1 minute)

```bash
# Create minimal config
cat > config/autonomous_training.yaml << EOF
autonomous_training:
  enabled: true
  evaluation:
    interval_seconds: 300
  triggers:
    performance_degradation:
      enabled: true
      min_degradation: 0.05
    scheduled:
      enabled: true
      force_retrain_after_days: 7
  resources:
    max_concurrent_training: 2
    cpu_limit_percent: 50
EOF
```

#### 4. Connect to DAA System (30 seconds)

```bash
# Register with DAA coordinator
curl -X POST http://localhost:8080/api/daa/register \
  -H "Content-Type: application/json" \
  -d '{
    "component": "autonomous_training",
    "endpoint": "http://training-coordinator:8090"
  }'
```

#### 5. Start Monitoring (2 minutes)

```bash
# Open Grafana dashboard
open http://localhost:3000/d/autonomous-training

# Check training status
curl http://localhost:8090/api/training/status
```

## Common Scenarios

### Scenario 1: Force Immediate Model Retraining

```bash
# Trigger manual retraining for a specific model
curl -X POST http://localhost:8090/api/training/manual \
  -H "Content-Type: application/json" \
  -d '{
    "model_id": "lstm_btc_predictor",
    "reason": "Market regime change detected",
    "priority": "high"
  }'
```

### Scenario 2: Adjust Performance Thresholds

```bash
# Lower threshold to trigger more frequent retraining
curl -X PATCH http://localhost:8090/api/config \
  -H "Content-Type: application/json" \
  -d '{
    "performance_threshold": 0.80,
    "drift_threshold": 0.08
  }'
```

### Scenario 3: View Training History

```bash
# Get last 10 training jobs
curl http://localhost:8090/api/training/history?limit=10

# Get specific job details
curl http://localhost:8090/api/training/job/{job_id}
```

### Scenario 4: Pause Autonomous Training

```bash
# Temporarily disable autonomous decisions
curl -X POST http://localhost:8090/api/training/pause

# Resume later
curl -X POST http://localhost:8090/api/training/resume
```

## Quick Monitoring Commands

### Check System Health

```bash
# Overall system status
curl http://localhost:8090/api/health

# Resource usage
curl http://localhost:8090/api/resources
```

### View Active Jobs

```bash
# List running training jobs
curl http://localhost:8090/api/training/active

# Get job progress
curl http://localhost:8090/api/training/job/{job_id}/progress
```

### Performance Metrics

```bash
# Current model performance
curl http://localhost:8090/api/models/performance

# Training improvement statistics
curl http://localhost:8090/api/training/stats
```

## Troubleshooting Guide

### Issue: Training Jobs Not Starting

**Symptom**: Jobs queued but not executing

**Quick Fix**:
```bash
# Check resource availability
docker stats training-coordinator

# Increase resource limits if needed
docker-compose exec training-coordinator \
  curl -X PATCH http://localhost:8090/api/config \
  -d '{"cpu_limit_percent": 75}'
```

### Issue: Poor Model Performance After Training

**Symptom**: Retrained models performing worse

**Quick Fix**:
```bash
# Enable stricter validation
curl -X PATCH http://localhost:8090/api/config \
  -d '{
    "safety.min_improvement": 0.02,
    "safety.rollback_on_failure": true
  }'
```

### Issue: Training Taking Too Long

**Symptom**: Jobs running for hours

**Quick Fix**:
```bash
# Reduce training data window
curl -X PATCH http://localhost:8090/api/config \
  -d '{"training.data_lookback_days": 30}'

# Enable GPU if available
docker-compose exec training-coordinator \
  nvidia-smi  # Check GPU availability
```

## Quick Configuration Reference

### Essential Environment Variables

```bash
# Core settings
AUTONOMOUS_TRAINING_ENABLED=true
TRAINING_EVALUATION_INTERVAL=300  # seconds
TRAINING_MAX_CONCURRENT_JOBS=2

# Performance thresholds
TRAINING_PERFORMANCE_THRESHOLD=0.85
TRAINING_DRIFT_THRESHOLD=0.1
TRAINING_MAX_MODEL_AGE_DAYS=7

# Resource limits
TRAINING_CPU_LIMIT_PERCENT=50
TRAINING_MEMORY_LIMIT_MB=8192
TRAINING_GPU_ENABLED=true

# Safety settings
TRAINING_REQUIRE_VALIDATION=true
TRAINING_MIN_IMPROVEMENT=0.01
TRAINING_AB_TEST_DURATION_HOURS=2
```

### API Endpoints Quick Reference

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/training/status` | GET | System status |
| `/api/training/manual` | POST | Trigger manual training |
| `/api/training/history` | GET | Training history |
| `/api/training/job/{id}` | GET | Job details |
| `/api/training/pause` | POST | Pause autonomous training |
| `/api/training/resume` | POST | Resume training |
| `/api/models/performance` | GET | Model performance metrics |
| `/api/config` | PATCH | Update configuration |

## Production Checklist

Before going to production, ensure:

- [ ] Resource limits configured appropriately
- [ ] Backup strategy for model artifacts
- [ ] Monitoring dashboards set up
- [ ] Alert rules configured
- [ ] Rollback procedures tested
- [ ] Training windows align with trading schedule
- [ ] Model validation thresholds tuned
- [ ] A/B testing duration appropriate

## Next Steps

1. **Fine-tune Thresholds**: Adjust triggers based on your specific models
2. **Set Up Alerts**: Configure notifications for training failures
3. **Customize Dashboard**: Add model-specific metrics to Grafana
4. **Schedule Maintenance**: Plan regular model performance reviews
5. **Document Patterns**: Track which triggers lead to best improvements

## Getting Help

- **Logs**: `docker-compose logs -f training-coordinator`
- **Metrics**: http://localhost:8090/metrics
- **API Docs**: http://localhost:8090/api/docs
- **Support**: Check `/products/features/neural-training/` for detailed guides

---

*Quick Start Version: 1.0.0*  
*Last Updated: July 25, 2025*