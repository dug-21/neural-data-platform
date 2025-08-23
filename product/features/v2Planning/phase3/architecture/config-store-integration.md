# Config-Store Integration Architecture

## Overview

This document details the integration of config-store into the Neural Trader Rust application, replacing all environment-based configuration with a centralized, type-safe configuration management system.

## Current Configuration Problems

### Environment Variable Issues
```rust
// Scattered throughout codebase
let db_url = env::var("DATABASE_URL").expect("DATABASE_URL not set");
let redis_url = env::var("REDIS_URL").unwrap_or("redis://localhost:6379".to_string());
let model_path = env::var("MODEL_PATH")?;
let api_key = env::var("ALPACA_API_KEY")?;
```

**Problems:**
- No type safety or validation
- Runtime failures on missing variables
- No centralized configuration management
- Difficult to test different configurations
- No runtime updates without restart

## Config-Store Architecture

### ConfigStoreService Integration
```rust
// src/infrastructure/config_store/client.rs
use tonic::transport::Channel;
use crate::proto::config_store::{
    config_store_service_client::ConfigStoreServiceClient,
    GetConfigRequest, GetBulkConfigRequest, WatchConfigRequest,
    ConfigValue, ConfigChangeEvent,
};

pub struct ConfigStoreClient {
    client: ConfigStoreServiceClient<Channel>,
    namespace: String,
    cache: Arc<RwLock<HashMap<String, CachedConfig>>>,
}

impl ConfigStoreClient {
    pub async fn new(url: &str, namespace: &str) -> Result<Self, ConfigError> {
        let channel = Channel::from_shared(url.to_string())?
            .connect()
            .await?;
        
        let client = ConfigStoreServiceClient::new(channel);
        
        Ok(Self {
            client,
            namespace: namespace.to_string(),
            cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T, ConfigError> {
        // Check cache first
        if let Some(cached) = self.get_cached(key).await {
            return Ok(cached);
        }
        
        // Fetch from config-store
        let request = GetConfigRequest {
            namespace_path: self.namespace.clone(),
            key: key.to_string(),
            include_metadata: false,
            ..Default::default()
        };
        
        let response = self.client
            .clone()
            .get_config(request)
            .await?
            .into_inner();
        
        if !response.success {
            return Err(ConfigError::NotFound(key.to_string()));
        }
        
        let value: T = serde_json::from_str(&response.value.unwrap().json_value)?;
        
        // Update cache
        self.update_cache(key, &value).await;
        
        Ok(value)
    }
    
    pub async fn watch(&self, keys: Vec<String>) -> Result<ConfigWatcher, ConfigError> {
        let request = WatchConfigRequest {
            namespace_path: self.namespace.clone(),
            keys,
            include_initial_values: true,
        };
        
        let stream = self.client
            .clone()
            .watch_config(request)
            .await?
            .into_inner();
        
        Ok(ConfigWatcher::new(stream, self.cache.clone()))
    }
}
```

## Configuration Schemas

### Service Configurations
```rust
// src/infrastructure/config_store/schemas.rs
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct TradingServiceConfig {
    #[validate(url)]
    pub database_url: String,
    
    #[validate(url)]
    pub redis_url: String,
    
    pub alpaca: AlpacaConfig,
    pub risk_limits: RiskLimitsConfig,
    pub trading_hours: TradingHoursConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct AlpacaConfig {
    #[validate(length(min = 1))]
    pub api_key: String,
    
    #[validate(length(min = 1))]
    pub api_secret: String,
    
    #[validate(url)]
    pub base_url: String,
    
    pub paper_trading: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct NeuralServiceConfig {
    #[validate(length(min = 1))]
    pub model_path: String,
    
    #[validate(range(min = 1, max = 10000))]
    pub input_size: usize,
    
    pub hidden_layers: Vec<usize>,
    
    #[validate(range(min = 0.0001, max = 1.0))]
    pub learning_rate: f64,
    
    #[validate(range(min = 1, max = 1000))]
    pub batch_size: usize,
    
    pub use_gpu: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct RiskLimitsConfig {
    #[validate(range(min = 0.01, max = 1.0))]
    pub max_position_size: f64,  // As percentage of portfolio
    
    #[validate(range(min = 0.001, max = 0.1))]
    pub max_daily_loss: f64,     // 0.1 = 10%
    
    #[validate(range(min = 0.01, max = 0.2))]
    pub stop_loss: f64,          // Per trade
    
    #[validate(range(min = 0.5, max = 1.0))]
    pub min_confidence: f64,     // Minimum signal confidence
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingHoursConfig {
    pub market_open: String,     // "09:30"
    pub market_close: String,    // "16:00"
    pub timezone: String,        // "America/New_York"
    pub trading_days: Vec<String>, // ["Mon", "Tue", "Wed", "Thu", "Fri"]
}
```

### Configuration Service
```rust
// src/infrastructure/config_store/service.rs
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct ConfigService {
    client: Arc<ConfigStoreClient>,
    trading_config: Arc<RwLock<Option<TradingServiceConfig>>>,
    neural_config: Arc<RwLock<Option<NeuralServiceConfig>>>,
    watcher_handle: Option<tokio::task::JoinHandle<()>>,
}

impl ConfigService {
    pub async fn new(config_store_url: &str) -> Result<Self, ConfigError> {
        let client = Arc::new(
            ConfigStoreClient::new(config_store_url, "/neural-trader").await?
        );
        
        let service = Self {
            client: client.clone(),
            trading_config: Arc::new(RwLock::new(None)),
            neural_config: Arc::new(RwLock::new(None)),
            watcher_handle: None,
        };
        
        // Load initial configurations
        service.reload_all().await?;
        
        // Start watching for changes
        service.start_watching().await?;
        
        Ok(service)
    }
    
    pub async fn reload_all(&self) -> Result<(), ConfigError> {
        // Load trading configuration
        let trading_config: TradingServiceConfig = self.client
            .get("trading")
            .await?;
        
        // Validate configuration
        trading_config.validate()
            .map_err(|e| ConfigError::Validation(e.to_string()))?;
        
        *self.trading_config.write().await = Some(trading_config);
        
        // Load neural configuration
        let neural_config: NeuralServiceConfig = self.client
            .get("neural")
            .await?;
        
        neural_config.validate()
            .map_err(|e| ConfigError::Validation(e.to_string()))?;
        
        *self.neural_config.write().await = Some(neural_config);
        
        Ok(())
    }
    
    pub async fn get_trading_config(&self) -> Result<TradingServiceConfig, ConfigError> {
        self.trading_config
            .read()
            .await
            .clone()
            .ok_or(ConfigError::NotLoaded("trading".to_string()))
    }
    
    pub async fn get_neural_config(&self) -> Result<NeuralServiceConfig, ConfigError> {
        self.neural_config
            .read()
            .await
            .clone()
            .ok_or(ConfigError::NotLoaded("neural".to_string()))
    }
    
    async fn start_watching(&mut self) -> Result<(), ConfigError> {
        let client = self.client.clone();
        let trading_config = self.trading_config.clone();
        let neural_config = self.neural_config.clone();
        
        let handle = tokio::spawn(async move {
            let mut watcher = client
                .watch(vec!["trading".to_string(), "neural".to_string()])
                .await
                .expect("Failed to create config watcher");
            
            while let Some(event) = watcher.next().await {
                match event {
                    Ok(ConfigChangeEvent { key, value, .. }) => {
                        match key.as_str() {
                            "trading" => {
                                if let Ok(config) = serde_json::from_str::<TradingServiceConfig>(&value) {
                                    if config.validate().is_ok() {
                                        *trading_config.write().await = Some(config);
                                        log::info!("Trading configuration updated");
                                    }
                                }
                            }
                            "neural" => {
                                if let Ok(config) = serde_json::from_str::<NeuralServiceConfig>(&value) {
                                    if config.validate().is_ok() {
                                        *neural_config.write().await = Some(config);
                                        log::info!("Neural configuration updated");
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        log::error!("Config watch error: {}", e);
                    }
                }
            }
        });
        
        self.watcher_handle = Some(handle);
        Ok(())
    }
}
```

## Dependency Injection with Config

### Application Layer Integration
```rust
// src/application/services/trading_service.rs
use crate::infrastructure::config_store::ConfigService;

pub struct TradingApplicationService {
    config_service: Arc<ConfigService>,
    signal_generator: Arc<dyn SignalGenerator>,
    risk_validator: Arc<dyn RiskValidator>,
}

impl TradingApplicationService {
    pub async fn new(config_service: Arc<ConfigService>) -> Result<Self, ServiceError> {
        let config = config_service.get_trading_config().await?;
        
        // Create components with configuration
        let signal_generator = Arc::new(
            NeuralSignalGenerator::new(
                config_service.get_neural_config().await?
            )
        );
        
        let risk_validator = Arc::new(
            DefaultRiskValidator::new(config.risk_limits)
        );
        
        Ok(Self {
            config_service,
            signal_generator,
            risk_validator,
        })
    }
    
    pub async fn reload_configuration(&self) -> Result<(), ServiceError> {
        self.config_service.reload_all().await?;
        
        // Update components with new configuration
        let config = self.config_service.get_trading_config().await?;
        self.risk_validator.update_limits(config.risk_limits);
        
        Ok(())
    }
}
```

### Main Application Bootstrap
```rust
// src/main.rs
use config_store::infrastructure::config_store::ConfigService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    
    // Get config-store URL from environment (only config needed from env)
    let config_store_url = env::var("CONFIG_STORE_URL")
        .unwrap_or_else(|_| "grpc://localhost:50051".to_string());
    
    // Initialize configuration service
    let config_service = Arc::new(
        ConfigService::new(&config_store_url).await?
    );
    
    // Get configurations
    let trading_config = config_service.get_trading_config().await?;
    let neural_config = config_service.get_neural_config().await?;
    
    // Initialize infrastructure with configuration
    let db_pool = PgPool::connect(&trading_config.database_url).await?;
    let redis_client = redis::Client::open(trading_config.redis_url.as_str())?;
    
    // Create repositories
    let signal_repo = Arc::new(PostgresSignalRepository::new(db_pool.clone()));
    let market_repo = Arc::new(RedisMarketDataRepository::new(redis_client.clone()));
    
    // Create application services
    let trading_service = TradingApplicationService::new(config_service.clone()).await?;
    
    // Create gRPC service
    let grpc_service = TradingServiceImpl::new(trading_service);
    
    // Start server
    let addr = "[::1]:50051".parse()?;
    log::info!("Starting gRPC server on {}", addr);
    
    Server::builder()
        .add_service(TradingServiceServer::new(grpc_service))
        .serve(addr)
        .await?;
    
    Ok(())
}
```

## Testing with Config-Store

### Mock Configuration for Tests
```rust
// src/infrastructure/config_store/mock.rs
use mockall::mock;

mock! {
    ConfigService {}
    
    impl ConfigService {
        async fn get_trading_config(&self) -> Result<TradingServiceConfig, ConfigError>;
        async fn get_neural_config(&self) -> Result<NeuralServiceConfig, ConfigError>;
        async fn reload_all(&self) -> Result<(), ConfigError>;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_with_mock_config() {
        let mut mock_config = MockConfigService::new();
        
        mock_config
            .expect_get_trading_config()
            .returning(|| Ok(TradingServiceConfig {
                database_url: "postgres://test".to_string(),
                redis_url: "redis://test".to_string(),
                alpaca: AlpacaConfig {
                    api_key: "test_key".to_string(),
                    api_secret: "test_secret".to_string(),
                    base_url: "https://paper-api.alpaca.markets".to_string(),
                    paper_trading: true,
                },
                risk_limits: RiskLimitsConfig {
                    max_position_size: 0.05,
                    max_daily_loss: 0.02,
                    stop_loss: 0.05,
                    min_confidence: 0.7,
                },
                trading_hours: TradingHoursConfig {
                    market_open: "09:30".to_string(),
                    market_close: "16:00".to_string(),
                    timezone: "America/New_York".to_string(),
                    trading_days: vec!["Mon".to_string(), "Tue".to_string()],
                },
            }));
        
        let service = TradingApplicationService::new(Arc::new(mock_config))
            .await
            .unwrap();
        
        // Test with mocked configuration
    }
}
```

### Integration Testing
```rust
#[cfg(test)]
mod integration_tests {
    use testcontainers::{clients, images::generic::GenericImage};
    
    #[tokio::test]
    async fn test_config_store_integration() {
        // Start config-store container
        let docker = clients::Cli::default();
        let config_store = docker.run(
            GenericImage::new("neural-trader/config-store", "latest")
                .with_exposed_port(50051)
        );
        
        let port = config_store.get_host_port(50051);
        let config_store_url = format!("grpc://localhost:{}", port);
        
        // Create config service
        let config_service = ConfigService::new(&config_store_url)
            .await
            .unwrap();
        
        // Test configuration loading
        let trading_config = config_service.get_trading_config().await;
        assert!(trading_config.is_ok());
        
        // Test configuration reload
        let result = config_service.reload_all().await;
        assert!(result.is_ok());
    }
}
```

## Migration Strategy

### Phase 1: Dual Configuration Support
```rust
// Temporary backward compatibility
pub fn get_database_url() -> String {
    // Try config-store first, fall back to env
    if let Ok(config) = CONFIG_SERVICE.get_trading_config().await {
        config.database_url
    } else {
        env::var("DATABASE_URL").expect("No configuration available")
    }
}
```

### Phase 2: Component Migration
1. Migrate configuration schemas
2. Update components to use ConfigService
3. Remove environment variable usage
4. Add configuration validation

### Phase 3: Complete Migration
1. Remove all env::var calls
2. Validate all configurations
3. Enable hot reloading
4. Document configuration schemas

## Configuration Management

### Config-Store Namespaces
```
/neural-trader/
├── trading/          # Trading service configuration
├── neural/           # Neural network configuration
├── risk/            # Risk management configuration
├── market-data/     # Market data configuration
└── monitoring/      # Monitoring configuration
```

### Secret Management
```rust
// Secrets are loaded from environment and stored in config-store
pub async fn initialize_secrets(config_service: &ConfigService) -> Result<(), ConfigError> {
    // Load secrets from environment
    let api_key = env::var("ALPACA_API_KEY_SECRET")?;
    let api_secret = env::var("ALPACA_API_SECRET_SECRET")?;
    
    // Store in config-store (encrypted)
    config_service.set_secret("alpaca.api_key", &api_key).await?;
    config_service.set_secret("alpaca.api_secret", &api_secret).await?;
    
    Ok(())
}
```

## Benefits

### Centralized Configuration
- Single source of truth
- Version controlled schemas
- Audit trail for changes
- Environment-agnostic

### Runtime Updates
- No service restarts required
- Gradual rollout of changes
- Instant rollback capability
- A/B testing support

### Type Safety
- Compile-time validation
- Runtime schema validation
- Strongly typed access
- IDE autocomplete

### Testing
- Easy mock configuration
- Consistent test environments
- Configuration-driven tests
- Integration test support

## Monitoring

### Configuration Metrics
```rust
// Prometheus metrics for configuration
lazy_static! {
    static ref CONFIG_LOADS: IntCounterVec = register_int_counter_vec!(
        "config_loads_total",
        "Total number of configuration loads",
        &["service", "status"]
    ).unwrap();
    
    static ref CONFIG_RELOAD_DURATION: HistogramVec = register_histogram_vec!(
        "config_reload_duration_seconds",
        "Configuration reload duration",
        &["service"]
    ).unwrap();
}
```

## Conclusion

Config-store integration provides:
- **Type-safe configuration** with validation
- **Runtime updates** without restarts
- **Centralized management** across services
- **Better testing** with mock configurations
- **Audit trail** for configuration changes