# Revised Weekend Implementation Plan - Neural Trader System
## Configuration-First Approach for Existing Infrastructure

### Executive Summary
This revised plan focuses on updating existing production infrastructure rather than deploying new services. Since Prometheus and Grafana are already running in production, we'll enhance their configurations and add new dashboards/alerts.

**Timeline**: Friday 6PM - Monday 9AM  
**Risk Level**: Low-Medium (configuration changes only)  
**Team Structure**: 2 engineers with DevOps experience  
**Success Criteria**: All features deployed with zero service interruptions  

---

## Key Changes from Original Plan

### What We're NOT Doing:
- ❌ Deploying new Prometheus/Grafana instances
- ❌ Creating duplicate observability infrastructure
- ❌ Major architectural changes
- ❌ Service migrations

### What We ARE Doing:
- ✅ Updating existing Prometheus configurations
- ✅ Adding new Grafana dashboards
- ✅ Configuring alerts in existing AlertManager
- ✅ Enhancing current services with new features
- ✅ Zero-downtime configuration updates

---

## Pre-Weekend Preparation (Thursday-Friday)

### Thursday Tasks (Complete by EOD)
```bash
# Configuration preparation
- [ ] Create all configuration files in feature branches
- [ ] Test configurations in dev environment
- [ ] Prepare rollback configurations
- [ ] Document all configuration changes
- [ ] Create configuration validation scripts

# Access verification
- [ ] Verify access to production Prometheus (port 9090)
- [ ] Verify access to production Grafana (port 3000)
- [ ] Test configuration reload endpoints
- [ ] Confirm backup procedures for configs
```

### Friday Afternoon (2PM-6PM)
```bash
# Final preparations
- [ ] Backup current configurations
  - docker exec prometheus cat /etc/prometheus/prometheus.yml > backups/prometheus.yml.backup
  - docker exec grafana tar -czf - /etc/grafana > backups/grafana.tar.gz
- [ ] Team sync on deployment approach
- [ ] Verify rollback procedures
- [ ] Set up monitoring for config changes
```

---

## Phase-by-Phase Implementation

### FRIDAY EVENING: Configuration Staging (6PM-10PM)

#### 6:00-7:00 PM: Backup and Validation
**Lead**: DevOps Engineer  
**Tasks**:
```bash
# 1. Create comprehensive backups
mkdir -p /backups/$(date +%Y%m%d)
cd /backups/$(date +%Y%m%d)

# Backup Prometheus configs
docker cp prometheus:/etc/prometheus ./prometheus-config-backup
docker cp prometheus:/prometheus/data ./prometheus-data-backup

# Backup Grafana configs
docker cp grafana:/var/lib/grafana ./grafana-data-backup
docker cp grafana:/etc/grafana ./grafana-config-backup

# 2. Validate current state
curl -s http://localhost:9090/-/healthy
curl -s http://localhost:3000/api/health

# 3. Document current metrics
curl -s http://localhost:9090/api/v1/targets | jq '.data.activeTargets | length'
```

#### 7:00-8:30 PM: Prometheus Configuration Updates
**Lead**: SRE Engineer  
**Configuration Updates**:
```yaml
# Update prometheus.yml with new scrape configs
# Using docker cp to update without restart

# 1. Add new scrape targets for enhanced services
cat << 'EOF' > prometheus-updates.yml
  # Alpaca WebSocket Metrics
  - job_name: 'alpaca-websocket'
    static_configs:
      - targets: ['neural-trader:3031']
    metrics_path: '/metrics/websocket'
    scrape_interval: 5s
    metric_relabel_configs:
      - source_labels: [__name__]
        regex: 'websocket_.*'
        action: keep

  # Neural Training Metrics
  - job_name: 'neural-training'
    static_configs:
      - targets: ['neural-trader:3032']
    metrics_path: '/metrics/neural'
    scrape_interval: 30s

  # Backfill Pipeline Metrics
  - job_name: 'backfill-pipeline'
    static_configs:
      - targets: ['data-ingestion:9091']
    metrics_path: '/metrics/backfill'
    scrape_interval: 10s
EOF

# 2. Apply configuration
docker exec prometheus sh -c 'cat >> /etc/prometheus/prometheus.yml' < prometheus-updates.yml

# 3. Reload configuration (no downtime)
curl -X POST http://localhost:9090/-/reload
```

#### 8:30-10:00 PM: Grafana Dashboard Updates
**Dashboard Deployment**:
```bash
# 1. Import new dashboards via API
for dashboard in dashboards/*.json; do
  curl -X POST http://admin:${GRAFANA_PASSWORD}@localhost:3000/api/dashboards/db \
    -H "Content-Type: application/json" \
    -d @"$dashboard"
done

# 2. Create alert rules
curl -X POST http://admin:${GRAFANA_PASSWORD}@localhost:3000/api/v1/provisioning/alert-rules \
  -H "Content-Type: application/json" \
  -d @"alerts/neural-trader-alerts.json"

# 3. Configure notification channels
curl -X POST http://admin:${GRAFANA_PASSWORD}@localhost:3000/api/alert-notifications \
  -H "Content-Type: application/json" \
  -d @"notifications/channels.json"
```

**New Dashboards to Add**:
- Alpaca WebSocket Health Dashboard
- Neural Training Progress Dashboard
- Backfill Pipeline Status Dashboard
- System Performance Overview (Enhanced)

---

### SATURDAY: Core Feature Deployment

#### Phase 1: Alpaca Reliability Enhancement (8AM-2PM)

##### 8:00-9:30 AM: Update Neural Trader Service
**Configuration Updates Only**:
```bash
# 1. Update environment variables for existing service
docker service update \
  --env-add ALPACA_CIRCUIT_BREAKER_ENABLED=true \
  --env-add ALPACA_CIRCUIT_BREAKER_THRESHOLD=5 \
  --env-add ALPACA_CIRCUIT_BREAKER_TIMEOUT=60 \
  --env-add ALPACA_RATE_LIMIT_ENABLED=true \
  --env-add ALPACA_RATE_LIMIT_PER_MINUTE=200 \
  --env-add ALPACA_CONNECTION_POOL_SIZE=10 \
  neural-trader

# 2. Verify configuration applied
docker service ps neural-trader
docker service logs neural-trader --tail 100

# 3. Monitor metrics endpoint
watch -n 5 'curl -s http://localhost:3030/metrics | grep alpaca_'
```

##### 9:30-11:00 AM: Configuration Validation
```bash
# Test circuit breaker
python tests/integration/test_alpaca_circuit_breaker.py

# Verify rate limiting
python tests/integration/test_alpaca_rate_limit.py

# Check WebSocket resilience
python tests/integration/test_websocket_reconnection.py
```

##### 11:00 AM-2:00 PM: Gradual Feature Rollout
```bash
# Enable features progressively via feature flags
curl -X POST http://localhost:3030/admin/features \
  -H "Content-Type: application/json" \
  -d '{
    "circuit_breaker": {"enabled": true, "rollout_percentage": 10},
    "enhanced_logging": {"enabled": true, "rollout_percentage": 100},
    "connection_pooling": {"enabled": true, "rollout_percentage": 50}
  }'

# Monitor impact
# Increase rollout percentage every 30 minutes if metrics are stable
```

#### Phase 2: File Backfill Configuration (2PM-6PM)

##### 2:00-3:30 PM: Configure Backfill Service
**No New Deployments - Update Existing Data Ingestion**:
```bash
# 1. Update data-ingestion service configuration
docker exec data-ingestion sh -c 'cat > /app/config/backfill.yaml' << 'EOF'
backfill:
  enabled: true
  storage_path: /mnt/market-data
  validation:
    max_error_rate: 0.01
    chunk_size: 10000
    parallel_workers: 4
  scheduling:
    enabled: true
    daily_run_time: "02:00"
    retry_failed: true
EOF

# 2. Mount external storage (if not already mounted)
docker service update \
  --mount-add type=bind,source=/mnt/market-data,target=/mnt/market-data \
  data-ingestion

# 3. Reload configuration
docker exec data-ingestion pkill -HUP python
```

##### 3:30-5:00 PM: Test Backfill Pipeline
```bash
# 1. Run test backfill
docker exec data-ingestion python -m data_ingestion.backfill \
  --test-mode \
  --start-date 2024-01-25 \
  --end-date 2024-01-25 \
  --symbols SPY

# 2. Monitor progress via existing Prometheus
curl -s http://localhost:9090/api/v1/query?query=backfill_records_processed

# 3. Check data quality
docker exec timescaledb psql -U postgres -d neural_trader -c "
  SELECT COUNT(*), MIN(timestamp), MAX(timestamp)
  FROM market_data_1m
  WHERE symbol = 'SPY' AND timestamp::date = '2024-01-25'
"
```

---

### SUNDAY: Advanced Features

#### Phase 3: Neural Training Configuration (8AM-2PM)

##### 8:00-10:00 AM: Enable Neural Features
**Update Existing Services**:
```bash
# 1. Enable neural training in neural-trader
docker service update \
  --env-add NEURAL_TRAINING_ENABLED=true \
  --env-add NEURAL_MODEL_PATH=/models \
  --env-add NEURAL_GPU_ENABLED=false \
  --env-add DAA_COORDINATOR_ENABLED=true \
  neural-trader

# 2. Create model storage volume if needed
docker volume create neural_models
docker service update \
  --mount-add type=volume,source=neural_models,target=/models \
  neural-trader

# 3. Initialize DAA coordinator
curl -X POST http://localhost:3030/api/daa/init \
  -H "Content-Type: application/json" \
  -d '{
    "topology": "hierarchical",
    "agents": ["pattern_recognizer", "risk_analyzer", "strategy_optimizer"]
  }'
```

##### 10:00 AM-12:00 PM: Configure Training Pipeline
```bash
# 1. Set up training schedule
curl -X POST http://localhost:3030/api/neural/schedule \
  -H "Content-Type: application/json" \
  -d '{
    "training_schedule": "0 2 * * *",
    "data_window": "30d",
    "models": ["lstm_predictor", "pattern_detector", "risk_assessor"]
  }'

# 2. Configure model parameters
curl -X POST http://localhost:3030/api/neural/config \
  -H "Content-Type: application/json" \
  -d @"config/neural_models.json"

# 3. Test training pipeline
curl -X POST http://localhost:3030/api/neural/train/test \
  -d '{"model": "lstm_predictor", "epochs": 1}'
```

#### Phase 4: Monitoring Enhancement (2PM-6PM)

##### 2:00-4:00 PM: Alert Configuration
```bash
# 1. Update Prometheus alerts
cat << 'EOF' > /tmp/neural-alerts.yml
groups:
  - name: neural_trader_enhanced
    rules:
      - alert: AlpacaWebSocketDisconnected
        expr: websocket_connection_state != 2
        for: 30s
        annotations:
          summary: "Alpaca WebSocket disconnected"
          
      - alert: BackfillPipelineFailed
        expr: backfill_error_rate > 0.01
        for: 5m
        annotations:
          summary: "Backfill error rate exceeded threshold"
          
      - alert: NeuralTrainingFailed
        expr: neural_training_success_rate < 0.95
        for: 10m
        annotations:
          summary: "Neural training failure rate high"
EOF

docker cp /tmp/neural-alerts.yml prometheus:/etc/prometheus/alerts/
curl -X POST http://localhost:9090/-/reload
```

##### 4:00-6:00 PM: Final Validation
```bash
# 1. Verify all services healthy
for service in neural-trader data-ingestion timescaledb redis nginx; do
  echo "Checking $service..."
  docker service ps $service --no-trunc
done

# 2. Test end-to-end flow
python tests/e2e/test_complete_pipeline.py

# 3. Generate deployment report
python scripts/generate_deployment_report.py \
  --config-changes-only \
  --output /reports/deployment_$(date +%Y%m%d).html
```

---

## Simplified Rollback Procedures

### Configuration Rollback (2 minutes)
```bash
# 1. Restore Prometheus config
docker cp backups/prometheus.yml prometheus:/etc/prometheus/prometheus.yml
curl -X POST http://localhost:9090/-/reload

# 2. Restore Grafana dashboards
# Use Grafana UI to import backup dashboards

# 3. Revert environment variables
docker service update --env-rm ALPACA_CIRCUIT_BREAKER_ENABLED neural-trader
# ... repeat for other env vars
```

### Feature Flag Rollback (30 seconds)
```bash
# Disable features instantly
curl -X POST http://localhost:3030/admin/features/disable-all
```

---

## Monitoring During Deployment

### Key Metrics to Watch
```bash
# Real-time monitoring dashboard
watch -n 5 '
echo "=== Service Health ==="
docker service ls
echo ""
echo "=== Prometheus Targets ==="
curl -s http://localhost:9090/api/v1/targets | jq ".data.activeTargets | length"
echo ""
echo "=== Recent Errors ==="
docker service logs neural-trader --tail 10 | grep ERROR
'
```

### Grafana Dashboards to Monitor
1. **Main Dashboard**: http://localhost:3000/d/main
2. **Alpaca Health**: http://localhost:3000/d/alpaca
3. **Backfill Status**: http://localhost:3000/d/backfill
4. **Neural Training**: http://localhost:3000/d/neural

---

## Success Criteria

### Configuration Success
- [ ] All configurations applied without service restarts
- [ ] No service interruptions during deployment
- [ ] All metrics flowing to Prometheus
- [ ] All dashboards loading in Grafana

### Feature Success  
- [ ] Alpaca circuit breaker activating correctly
- [ ] Backfill pipeline processing data
- [ ] Neural training completing test runs
- [ ] All alerts configured and tested

### Performance Success
- [ ] No performance degradation
- [ ] Memory usage stable
- [ ] CPU usage within limits
- [ ] Network traffic normal

---

## Post-Implementation Tasks (Monday)

### Morning Validation (6AM-9AM)
```bash
# 1. Pre-market checks
./scripts/pre_market_validation.sh

# 2. Verify configurations persisted
docker exec prometheus cat /etc/prometheus/prometheus.yml
docker exec neural-trader env | grep ALPACA

# 3. Check overnight metrics
python scripts/analyze_overnight_metrics.py
```

### Documentation Updates
- [ ] Update configuration management docs
- [ ] Document new Grafana dashboards
- [ ] Update runbooks with new alerts
- [ ] Create configuration backup procedures

---

## Emergency Contacts

| Role | Name | Phone | Responsibility |
|------|------|-------|----------------|
| DevOps Lead | Available | On-call | Configuration issues |
| Platform Engineer | Available | On-call | Service issues |
| SRE Manager | Available | Escalation | Major incidents |

---

## Key Differences Summary

1. **No New Services**: We're updating existing services only
2. **Configuration Focus**: All changes via env vars and config files  
3. **Zero Downtime**: Using reload endpoints and rolling updates
4. **Simpler Rollback**: Just restore configs, no service changes
5. **Lower Risk**: No architectural changes or new dependencies

This approach significantly reduces risk while achieving all the feature goals.