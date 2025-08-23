# Config Store Migration Guide

This document outlines the migration process for transitioning Neural Trader from environment-based configuration to the centralized config-store system.

## Overview

The config-store migration separates configuration management from secrets management, providing:
- **Configuration Store**: Non-sensitive configuration data with hot-reloading
- **Environment Variables**: Secrets and sensitive authentication data
- **Hierarchical Namespaces**: Organized configuration by domain and service

## Migration Strategy

### Phase 1: Setup and Preparation

1. **Deploy config-store service**
   ```bash
   docker-compose up -d config-store
   ```

2. **Run migration script**
   ```bash
   # Dry run first to preview changes
   python scripts/migrate_config.py --dry-run
   
   # Actual migration
   python scripts/migrate_config.py
   ```

3. **Verify migration**
   ```bash
   # Check Redis for populated configuration
   redis-cli -h redis keys "config::*"
   ```

### Phase 2: Service Integration

Update each service to use config-store:

#### Data Ingestion Service Example

**Before** (environment variables only):
```yaml
environment:
  PRIMARY_PROVIDER: alpaca
  SYMBOLS: AAPL,MSFT,GOOGL,AMZN,NVDA
  UPDATE_INTERVAL: 60s
  ALPACA_API_KEY: ${ALPACA_API_KEY}
```

**After** (config-store + environment secrets):
```yaml
environment:
  CONFIG_STORE_URL: http://config-store:8003
  CONFIG_NAMESPACE: neural-trading/data-ingestion
  # Secrets still from environment
  ALPACA_API_KEY: ${ALPACA_API_KEY}
  ALPACA_API_SECRET: ${ALPACA_API_SECRET}
  # URLs that can be overridden
  ALPACA_API_URL: ${ALPACA_API_URL:-https://paper-api.alpaca.markets}
```

#### Rust Service Integration

```rust
use config_store::{ConfigStore, ServiceConfig};

pub struct DataIngestionService {
    config: ServiceConfig<DataIngestionConfig>,
    // ... other fields
}

impl DataIngestionService {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Load configuration from config-store
        let config_store = ConfigStore::connect("http://config-store:8003").await?;
        let config = ServiceConfig::new(
            config_store,
            "neural-trading/data-ingestion",
            Box::new(DataIngestionConfigValidator),
        );
        
        let loaded_config = config.load().await?;
        
        // Apply secrets from environment
        let mut final_config = loaded_config.clone();
        final_config.sources.primary.api_key = std::env::var("ALPACA_API_KEY")?;
        final_config.sources.primary.api_secret = std::env::var("ALPACA_API_SECRET")?;
        
        Ok(Self { config, /* ... */ })
    }
    
    pub async fn start_config_watcher(&self) -> Result<(), ConfigError> {
        let config = self.config.clone();
        tokio::spawn(async move {
            let mut watcher = config.watch_changes().await.unwrap();
            while let Some(change) = watcher.next().await {
                if config.refresh().await.unwrap_or(false) {
                    log::info!("Configuration updated: {:?}", change);
                    // Apply configuration changes
                }
            }
        });
        Ok(())
    }
}
```

## Configuration Namespace Structure

### Neural Platform Shared Configurations

```
/neural-platform/shared/
├── eventbus/           # Redis Streams configuration
├── ml-ops/            # ML platform settings  
└── monitoring/        # Observability configuration
```

### Neural Trading Domain Configurations

```
/neural-trading/
├── data-ingestion/    # Data source and processing settings
├── model-execution/   # ML model configurations
└── action-layer/      # Trading and risk management
```

## Configuration Types

### Configuration Store (Non-Sensitive)
- API endpoints and URLs
- Timeouts and limits
- Feature flags
- Business logic parameters
- Schema definitions
- Processing settings

### Environment Variables (Sensitive)
- API keys and tokens
- Database passwords
- Authentication secrets
- Encryption keys
- Third-party credentials

## Migration Script Usage

### Basic Usage
```bash
# Run with defaults
python scripts/migrate_config.py

# Custom Redis URL
python scripts/migrate_config.py --redis-url redis://custom-redis:6379

# Dry run to preview changes
python scripts/migrate_config.py --dry-run

# Custom output files
python scripts/migrate_config.py \
  --seed-file /path/to/seed.json \
  --report-file /path/to/report.json
```

### Script Output
- **Seed File**: Initial configuration data in JSON format
- **Migration Report**: Summary of migration success/failures
- **Redis Population**: Configuration data stored in Redis

## Validation and Testing

### 1. Configuration Loading Test
```rust
#[tokio::test]
async fn test_config_loading() {
    let config_store = ConfigStore::connect("http://config-store:8003").await.unwrap();
    let config = config_store.get_namespace("neural-trading/data-ingestion").await.unwrap();
    
    assert!(config.contains_key("sources"));
    assert!(config.contains_key("validation"));
    assert!(config.contains_key("processing"));
}
```

### 2. Hot Reload Test
```rust
#[tokio::test]
async fn test_hot_reload() {
    let mut service = DataIngestionService::new().await.unwrap();
    service.start_config_watcher().await.unwrap();
    
    // Simulate configuration change
    let config_store = ConfigStore::connect("http://config-store:8003").await.unwrap();
    config_store.update_config("neural-trading/data-ingestion", updated_config).await.unwrap();
    
    // Wait for hot reload
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    // Verify configuration updated
    let current_config = service.get_current_config().await;
    assert_eq!(current_config.some_setting, expected_value);
}
```

### 3. Environment Variable Override Test
```bash
# Test that secrets are loaded from environment
export ALPACA_API_KEY="test_key"
export ALPACA_API_SECRET="test_secret"

# Run service and verify secrets are applied
cargo test test_secret_loading
```

## Monitoring and Observability

### Metrics
- `config_requests_total{method, status}` - Configuration access requests
- `config_cache_hits_total` - Cache hit rate
- `config_validation_errors_total{schema, error_type}` - Validation failures
- `config_changes_total{namespace, change_type}` - Configuration changes

### Health Checks
- Config-store service health via gRPC
- Redis connectivity
- Configuration schema validation
- Service configuration loading status

### Alerting
- Configuration service unavailable
- High configuration validation error rate
- Configuration change propagation delays
- Schema version conflicts

## Rollback Procedures

### Emergency Rollback
If config-store fails, services fall back to:
1. Cached configuration values
2. Environment variable overrides
3. Default configuration values

### Manual Rollback Steps
```bash
# 1. Stop config-store service
docker-compose stop config-store

# 2. Revert service configurations to use environment variables
# Edit docker-compose.yml to remove CONFIG_STORE_URL

# 3. Restart services
docker-compose up -d

# 4. Verify services are running with environment-based config
docker-compose ps
```

## Security Considerations

### Configuration Store
- No sensitive data stored
- Network isolation via Docker networks
- Read-only filesystem in production
- Regular security audits

### Environment Variables
- Secrets managed via Docker secrets or external secret managers
- No secrets in config-store or logs
- Encrypted at rest and in transit
- Access control via namespace isolation

## Best Practices

### Configuration Design
1. **Separate concerns**: Config vs. secrets
2. **Use defaults**: Provide sensible default values
3. **Validate schemas**: Enforce configuration structure
4. **Document changes**: Audit trail for all modifications

### Development Workflow
1. **Local development**: Use config_store_seed.json for initial setup
2. **Testing**: Mock config-store for unit tests
3. **Staging**: Full integration testing with config-store
4. **Production**: Gradual rollout with monitoring

### Operational Procedures
1. **Monitor health**: Config-store and dependent services
2. **Backup configuration**: Regular Redis snapshots
3. **Version control**: Track configuration schema changes
4. **Access control**: Limit who can modify configuration

## Troubleshooting

### Common Issues

#### Configuration Loading Failures
```
Error: Failed to connect to config-store
Solution: Check config-store service health and network connectivity
```

#### Schema Validation Errors
```
Error: Configuration does not match schema
Solution: Validate configuration against registered schema
```

#### Hot Reload Not Working
```
Error: Configuration changes not applied
Solution: Check Redis pub/sub and service watcher implementation
```

### Debugging Commands

```bash
# Check config-store logs
docker logs neural-trader-config-store

# Verify Redis configuration
redis-cli -h redis keys "config::*"
redis-cli -h redis hgetall "config::neural-trading/data-ingestion"

# Test gRPC connectivity
grpc_health_probe -addr=config-store:8003

# Validate service configuration loading
curl -H "Content-Type: application/json" \
  -d '{"namespace":"neural-trading/data-ingestion"}' \
  http://config-store:8003/config
```

## Future Enhancements

### Phase 3: Advanced Features
- Configuration versioning and rollback
- A/B testing support
- Configuration drift detection
- Automated configuration updates

### Phase 4: Enterprise Features
- Multi-environment configuration management
- Advanced access control and permissions
- Integration with external configuration systems
- Configuration compliance reporting

## Support and Documentation

- **Technical Issues**: Check logs and health endpoints
- **Configuration Schema**: Refer to schema registry
- **Best Practices**: Follow configuration design guidelines
- **Migration Support**: Run migration script with --dry-run first

For additional support, consult the config-store service documentation and monitoring dashboards.