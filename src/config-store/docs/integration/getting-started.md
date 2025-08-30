# ConfigStore Getting Started Guide

This guide provides a step-by-step walkthrough for integrating the ConfigStore system into your service, based on the V2 implementation plan's foundation-first approach.

## Quick Start

### 1. Add Dependencies

Add the config-store dependencies to your `Cargo.toml`:

```toml
[dependencies]
config-store = { path = "../config-store" }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
async-trait = "0.1"
thiserror = "1.0"
```

### 2. Define Your Configuration Structure

Create a configuration struct that represents your service's settings:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyServiceConfig {
    pub database_url: String,
    pub max_connections: u32,
    pub timeout_seconds: u64,
    pub feature_flags: FeatureFlags,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    pub enable_caching: bool,
    pub enable_metrics: bool,
    pub experimental_features: bool,
}

impl Default for MyServiceConfig {
    fn default() -> Self {
        Self {
            database_url: "postgresql://localhost:5432/mydb".to_string(),
            max_connections: 10,
            timeout_seconds: 30,
            feature_flags: FeatureFlags {
                enable_caching: true,
                enable_metrics: true,
                experimental_features: false,
            },
        }
    }
}
```

### 3. Set Up Configuration Validation

Create validators to ensure your configuration is valid:

```rust
use config_store::{Validator, ValidationError};

pub struct MyServiceConfigValidator;

impl Validator<MyServiceConfig> for MyServiceConfigValidator {
    fn validate(&self, config: &MyServiceConfig) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        // Validate max_connections
        if config.max_connections == 0 || config.max_connections > 1000 {
            errors.push(ValidationError {
                field: "max_connections".to_string(),
                message: "Must be between 1 and 1000".to_string(),
                code: "INVALID_RANGE".to_string(),
            });
        }

        // Validate timeout
        if config.timeout_seconds > 300 {
            errors.push(ValidationError {
                field: "timeout_seconds".to_string(),
                message: "Timeout cannot exceed 300 seconds".to_string(),
                code: "TIMEOUT_TOO_LARGE".to_string(),
            });
        }

        // Validate database URL format
        if !config.database_url.starts_with("postgresql://") {
            errors.push(ValidationError {
                field: "database_url".to_string(),
                message: "Must be a valid PostgreSQL URL".to_string(),
                code: "INVALID_URL_FORMAT".to_string(),
            });
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
```

### 4. Initialize Your Service with ConfigStore

Integrate the ConfigStore into your service:

```rust
use std::sync::Arc;
use config_store::{ConfigStore, ServiceConfig, ConfigError};

pub struct MyService {
    config: ServiceConfig<MyServiceConfig>,
    // ... other service fields
}

impl MyService {
    pub async fn new(config_store: Arc<dyn ConfigStore>) -> Result<Self, ConfigError> {
        // Create service configuration
        let config = ServiceConfig::new(
            config_store,
            "my_service", // Configuration path in the store
            Box::new(MyServiceConfigValidator),
        );

        // Load and validate initial configuration
        config.load().await?;

        Ok(Self { config })
    }

    pub async fn get_database_url(&self) -> Result<String, ConfigError> {
        let config = self.config.get().await?;
        Ok(config.database_url)
    }

    pub async fn get_max_connections(&self) -> Result<u32, ConfigError> {
        let config = self.config.get().await?;
        Ok(config.max_connections)
    }

    pub async fn is_caching_enabled(&self) -> Result<bool, ConfigError> {
        let config = self.config.get().await?;
        Ok(config.feature_flags.enable_caching)
    }

    pub async fn refresh_config(&self) -> Result<bool, ConfigError> {
        self.config.refresh().await
    }
}
```

### 5. Choose Your ConfigStore Implementation

#### For Development (File-based)
```rust
use config_store::FileConfigStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_store = Arc::new(
        FileConfigStore::new("./config")
    );
    
    let service = MyService::new(config_store).await?;
    
    // Use your service...
    Ok(())
}
```

#### For Production (Redis-based)
```rust
use config_store::RedisConfigStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_store = Arc::new(
        RedisConfigStore::new("redis://localhost:6379").await?
    );
    
    let service = MyService::new(config_store).await?;
    
    // Use your service...
    Ok(())
}
```

#### For Testing (In-Memory)
```rust
use config_store::{InMemoryConfigStore, ConfigValue};

#[tokio::test]
async fn test_my_service() {
    let config_store = Arc::new(InMemoryConfigStore::new());
    
    // Set up test configuration
    let test_config = serde_json::json!({
        "database_url": "postgresql://test:5432/testdb",
        "max_connections": 5,
        "timeout_seconds": 10,
        "feature_flags": {
            "enable_caching": false,
            "enable_metrics": true,
            "experimental_features": true
        }
    });
    
    config_store.set("my_service", ConfigValue::from(test_config)).await.unwrap();
    
    let service = MyService::new(config_store).await.unwrap();
    
    assert_eq!(service.get_max_connections().await.unwrap(), 5);
    assert!(!service.is_caching_enabled().await.unwrap());
}
```

## Example Implementations

### Neural Network Service Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralNetworkConfig {
    pub model_path: String,
    pub input_size: usize,
    pub hidden_layers: Vec<usize>,
    pub learning_rate: f64,
    pub batch_size: usize,
    pub epochs: usize,
    pub use_gpu: bool,
}

pub struct NeuralNetworkConfigValidator;

impl Validator<NeuralNetworkConfig> for NeuralNetworkConfigValidator {
    fn validate(&self, config: &NeuralNetworkConfig) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        if config.input_size == 0 {
            errors.push(ValidationError {
                field: "input_size".to_string(),
                message: "Input size must be greater than 0".to_string(),
                code: "INVALID_INPUT_SIZE".to_string(),
            });
        }

        if config.learning_rate <= 0.0 || config.learning_rate > 1.0 {
            errors.push(ValidationError {
                field: "learning_rate".to_string(),
                message: "Learning rate must be between 0 and 1".to_string(),
                code: "INVALID_LEARNING_RATE".to_string(),
            });
        }

        if config.batch_size == 0 {
            errors.push(ValidationError {
                field: "batch_size".to_string(),
                message: "Batch size must be greater than 0".to_string(),
                code: "INVALID_BATCH_SIZE".to_string(),
            });
        }

        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }
}
```

### Trading Service Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingConfig {
    pub api_key: String,
    pub base_url: String,
    pub risk_limits: RiskLimits,
    pub trading_hours: TradingHours,
    pub symbols: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskLimits {
    pub max_position_size: f64,
    pub max_daily_loss: f64,
    pub stop_loss_percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingHours {
    pub start: String, // "09:30"
    pub end: String,   // "16:00"
    pub timezone: String, // "America/New_York"
}
```

## Migration from .env Files

If you're currently using `.env` files, here's how to migrate:

### Before (using .env)
```bash
# .env
DATABASE_URL=postgresql://localhost:5432/mydb
MAX_CONNECTIONS=10
TIMEOUT_SECONDS=30
ENABLE_CACHING=true
```

```rust
// Old approach
use std::env;

fn load_config() -> MyServiceConfig {
    MyServiceConfig {
        database_url: env::var("DATABASE_URL").unwrap(),
        max_connections: env::var("MAX_CONNECTIONS").unwrap().parse().unwrap(),
        timeout_seconds: env::var("TIMEOUT_SECONDS").unwrap().parse().unwrap(),
        feature_flags: FeatureFlags {
            enable_caching: env::var("ENABLE_CACHING").unwrap().parse().unwrap(),
            enable_metrics: true,
            experimental_features: false,
        },
    }
}
```

### After (using ConfigStore)

1. **Convert .env to JSON configuration:**
```json
{
  "my_service": {
    "database_url": "postgresql://localhost:5432/mydb",
    "max_connections": 10,
    "timeout_seconds": 30,
    "feature_flags": {
      "enable_caching": true,
      "enable_metrics": true,
      "experimental_features": false
    }
  }
}
```

2. **Load with ConfigStore:**
```rust
// New approach with validation and type safety
pub async fn load_config(config_store: Arc<dyn ConfigStore>) -> Result<MyServiceConfig, ConfigError> {
    let config = ServiceConfig::new(
        config_store,
        "my_service",
        Box::new(MyServiceConfigValidator),
    );
    config.load().await
}
```

### Gradual Migration Strategy

You can migrate gradually by creating a hybrid approach:

```rust
pub struct HybridConfigStore {
    inner: Arc<dyn ConfigStore>,
    env_fallback: bool,
}

impl HybridConfigStore {
    pub fn new(inner: Arc<dyn ConfigStore>) -> Self {
        Self {
            inner,
            env_fallback: true,
        }
    }
}

#[async_trait]
impl ConfigStore for HybridConfigStore {
    async fn get(&self, path: &str) -> Result<ConfigValue, ConfigError> {
        match self.inner.get(path).await {
            Ok(value) => Ok(value),
            Err(_) if self.env_fallback => {
                // Fallback to environment variables
                match path {
                    "my_service.database_url" => {
                        std::env::var("DATABASE_URL")
                            .map(ConfigValue::String)
                            .map_err(|_| ConfigError::NotFound { path: path.to_string() })
                    }
                    "my_service.max_connections" => {
                        std::env::var("MAX_CONNECTIONS")
                            .and_then(|s| s.parse().map_err(|_| std::env::VarError::NotPresent))
                            .map(|n: u32| ConfigValue::Number(n.into()))
                            .map_err(|_| ConfigError::NotFound { path: path.to_string() })
                    }
                    _ => Err(ConfigError::NotFound { path: path.to_string() })
                }
            }
            Err(e) => Err(e),
        }
    }

    // ... implement other methods
}
```

## Configuration File Examples

### Development Configuration
```json
{
  "my_service": {
    "database_url": "postgresql://localhost:5432/mydb_dev",
    "max_connections": 5,
    "timeout_seconds": 10,
    "feature_flags": {
      "enable_caching": false,
      "enable_metrics": true,
      "experimental_features": true
    }
  }
}
```

### Production Configuration
```json
{
  "my_service": {
    "database_url": "postgresql://prod-db:5432/mydb",
    "max_connections": 100,
    "timeout_seconds": 30,
    "feature_flags": {
      "enable_caching": true,
      "enable_metrics": true,
      "experimental_features": false
    }
  }
}
```

### Testing Configuration
```json
{
  "my_service": {
    "database_url": "postgresql://localhost:5432/test_db",
    "max_connections": 1,
    "timeout_seconds": 5,
    "feature_flags": {
      "enable_caching": false,
      "enable_metrics": false,
      "experimental_features": true
    }
  }
}
```

## Best Practices

1. **Start Simple**: Begin with basic configuration and add complexity as needed
2. **Use Strong Types**: Define configuration structs rather than using raw JSON
3. **Validate Early**: Catch configuration errors at startup, not during runtime
4. **Test Configuration**: Write unit tests for your configuration logic
5. **Version Your Schema**: Include version information in your configuration
6. **Document Schema**: Maintain clear documentation of all configuration options
7. **Use Environment Overrides**: Allow environment variables to override file-based config
8. **Monitor Changes**: Log configuration changes and validation failures

## Common Patterns

### Hot Reloading Configuration
```rust
impl MyService {
    pub async fn start_config_watcher(&self) -> Result<(), ConfigError> {
        let config = self.config.clone();
        
        tokio::spawn(async move {
            let mut watcher = config.watch_changes().await.unwrap();
            
            while let Some(_change) = watcher.next().await {
                match config.refresh().await {
                    Ok(true) => {
                        log::info!("Configuration reloaded successfully");
                    }
                    Ok(false) => {
                        log::debug!("Configuration unchanged");
                    }
                    Err(e) => {
                        log::error!("Failed to reload configuration: {}", e);
                    }
                }
            }
        });
        
        Ok(())
    }
}
```

### Configuration with Secrets
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigWithSecrets {
    pub database_url: String,
    #[serde(skip_serializing)] // Don't log secrets
    pub api_key: String,
    pub public_config: PublicConfig,
}

// Use environment variables for secrets
impl ConfigWithSecrets {
    pub fn apply_secrets(&mut self) -> Result<(), ConfigError> {
        if let Ok(api_key) = std::env::var("API_KEY") {
            self.api_key = api_key;
        }
        
        if let Ok(db_url) = std::env::var("DATABASE_URL") {
            self.database_url = db_url;
        }
        
        Ok(())
    }
}
```

This getting started guide provides a foundation for integrating ConfigStore into your services while following the V2 implementation plan's principles of testability, validation, and clean architecture.