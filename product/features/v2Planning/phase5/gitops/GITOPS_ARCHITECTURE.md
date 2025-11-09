# GitOps Architecture for Neural Trader V2

## Executive Summary

This document defines the GitOps architecture for managing non-secret configuration across dev, test, and production environments for the Neural Trader V2 microservice ecosystem.

## Core Principles

1. **Single Source of Truth**: Git repository as the authoritative source for all non-secret configuration
2. **Environment Parity**: Clear visibility of configuration differences across environments
3. **Automated Synchronization**: Config-store auto-pulls from Git on startup
4. **Version Control**: Full audit trail of configuration changes through Git history

## Repository Structure

```
neural-trader/
├── config/                          # GitOps configuration root
│   ├── dev/                        # Development environment
│   │   ├── common/                 # Shared config across all services
│   │   │   ├── database.yaml
│   │   │   ├── redis.yaml
│   │   │   └── monitoring.yaml
│   │   ├── services/               # Service-specific configuration
│   │   │   ├── config-store.yaml
│   │   │   ├── data-ingestion.yaml
│   │   │   ├── data-staging.yaml
│   │   │   ├── neural-ml-ops.yaml
│   │   │   └── neural-trading.yaml
│   │   └── environment.yaml       # Environment metadata
│   │
│   ├── test/                       # Test environment (production-like)
│   │   ├── common/
│   │   ├── services/
│   │   └── environment.yaml
│   │
│   ├── prod/                       # Production environment
│   │   ├── common/
│   │   ├── services/
│   │   └── environment.yaml
│   │
│   └── schemas/                    # Configuration schemas
│       ├── service-config.schema.json
│       └── common-config.schema.json
```

## Configuration Files Format

### Service Configuration Example (config/dev/services/neural-trading.yaml)
```yaml
version: "1.0.0"
service: neural-trading
environment: dev

# Trading configuration
trading:
  capital: 100000  # Paper trading capital
  risk_limits:
    position_size_pct: 5.0
    daily_loss_pct: 2.0
    stop_loss_pct: 5.0
  
# EventBus configuration  
eventbus:
  channels:
    - "ml:features:*"
    - "trading:signals:*"
  batch_size: 100
  timeout_ms: 5000

# Model inference
inference:
  cache_size: 1000
  model_timeout_ms: 50
  
# DAA Coordinator
daa:
  consensus_threshold: 0.6
  voting_timeout_ms: 100
  max_agents: 5
```

### Common Configuration Example (config/dev/common/redis.yaml)
```yaml
version: "1.0.0"
component: redis
environment: dev

# Redis configuration for EventBus
eventbus:
  host: redis
  port: 6379
  database: 0
  stream_prefix: "eventbus:"
  consumer_group: "neural-trader"
  
# Redis configuration for raw data
raw_data:
  host: redis
  port: 6379
  database: 1
  channel_prefix: "market:"
```

## Config-Store Integration

### Seeding Strategy

1. **Startup Sequence**:
   ```bash
   # Config-store starts first and seeds from Git
   CONFIG_REPO_URL=https://github.com/org/neural-trader.git
   CONFIG_BRANCH=main
   CONFIG_ENV=dev
   
   # Config-store pulls configuration
   git clone --depth 1 $CONFIG_REPO_URL /tmp/config
   cd /tmp/config
   config-store seed --env $CONFIG_ENV --path config/$CONFIG_ENV
   ```

2. **Service Registration**:
   - Each service registers with config-store on startup
   - Receives initial configuration bundle
   - Subscribes to configuration updates (future enhancement)

### Configuration API

```proto
service ConfigStore {
  rpc GetServiceConfig(ServiceConfigRequest) returns (ServiceConfigResponse);
  rpc GetCommonConfig(CommonConfigRequest) returns (CommonConfigResponse);
  rpc WatchConfig(WatchConfigRequest) returns (stream ConfigUpdate);
}
```

## Environment Management

### Environment Variables (.env files)
```bash
# .env.dev - Development secrets (git-ignored)
ALPACA_API_KEY=PKxxxxxxxxxxxxx
ALPACA_SECRET_KEY=xxxxxxxxxxxxxxxx
POSTGRES_PASSWORD=dev_password
JWT_SECRET=dev_jwt_secret

# Config-store Git settings (can be in docker-compose)
CONFIG_REPO_URL=https://github.com/org/neural-trader.git
CONFIG_BRANCH=main
CONFIG_ENV=dev
```

### Docker Compose Environment Injection
```yaml
services:
  config-store:
    environment:
      - CONFIG_REPO_URL=${CONFIG_REPO_URL}
      - CONFIG_ENV=${CONFIG_ENV}
    env_file:
      - .env.${CONFIG_ENV}
```

## Configuration Promotion Workflow

### Development → Test → Production

1. **Development Changes**:
   - Edit `config/dev/` files
   - Test locally with dev environment
   - Commit and push to feature branch

2. **Test Promotion**:
   - Copy validated dev configs to `config/test/`
   - Adjust values for test environment
   - Run full CICD pipeline with test configs
   - Validate regression tests pass

3. **Production Promotion**:
   - After test validation, copy to `config/prod/`
   - Adjust for production scale/requirements
   - Create PR for review
   - Tag release after merge

## Version Control Strategy

### Branch Model
```
main
├── config/dev/     # Active development configs
├── config/test/    # Stable test configs  
└── config/prod/    # Production configs

feature/update-trading-limits
└── config/dev/     # Feature branch dev configs
```

### Tagging Strategy
```bash
# Tag production config releases
git tag -a config-v1.0.0 -m "Production config release 1.0.0"

# Tag test environment configs
git tag -a config-test-v1.0.0 -m "Test config release 1.0.0"
```

## Security Considerations

### Non-Secret Data Only
✅ **Stored in Git**:
- Service URLs and ports
- Feature flags
- Rate limits
- Batch sizes
- Timeouts
- Business logic parameters

❌ **NOT Stored in Git**:
- API keys
- Passwords
- JWT secrets
- Private certificates
- Encryption keys

### Access Control
- Read-only access for config-store service account
- Branch protection for `config/prod/`
- PR reviews required for production changes
- Audit logging of all config changes

## Monitoring & Alerting

### Configuration Drift Detection
```yaml
# config-monitor service (future)
drift_detection:
  enabled: true
  check_interval: 300  # seconds
  alert_on_drift: true
  auto_remediate: false
```

### Metrics to Track
- Config load failures
- Config validation errors
- Drift detection alerts
- Config update latency
- Service registration failures

## Implementation Phases

### Phase 1: Foundation (Current)
- Basic GitOps structure
- Config-store Git integration
- Manual config updates

### Phase 2: Automation
- Automated config validation
- CI checks for config changes
- Config diffing tools

### Phase 3: Advanced Features
- Hot-reload capability
- Config versioning API
- Rollback mechanisms
- A/B configuration testing

## Developer Workflow

1. **Local Development**:
   ```bash
   # Start config-store with dev configs
   CONFIG_ENV=dev docker-compose up config-store
   
   # Services auto-connect and load configs
   docker-compose up -d
   ```

2. **Config Changes**:
   ```bash
   # Edit configuration
   vim config/dev/services/neural-trading.yaml
   
   # Restart config-store to reload
   docker-compose restart config-store
   ```

3. **Testing Changes**:
   ```bash
   # Run with test configs
   CONFIG_ENV=test make cicd-pipeline
   ```

## Best Practices

1. **Keep Secrets Separate**: Never commit secrets to Git
2. **Use Schemas**: Validate configs against schemas
3. **Document Changes**: Clear commit messages for config updates
4. **Test First**: Always test in dev before promoting
5. **Incremental Changes**: Small, focused config updates
6. **Review Process**: Peer review for test/prod changes

## Tooling

### Config Validation
```bash
# Validate all configs against schemas
make validate-configs

# Diff configs between environments
make diff-configs ENV1=dev ENV2=test
```

### Config Generation
```bash
# Generate service config from template
make generate-config SERVICE=new-service ENV=dev
```

## Migration from V1

Since V2 is greenfield:
1. No migration of V1 configs needed
2. Start fresh with clean GitOps structure
3. Document lessons learned from V1

## Next Steps

1. Create initial config files for all services
2. Set up config-store Git integration
3. Define configuration schemas
4. Implement validation pipeline
5. Document service-specific configurations