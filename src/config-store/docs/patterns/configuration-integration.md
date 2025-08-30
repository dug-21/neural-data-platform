# Configuration Integration Patterns

This document defines the standard patterns for integrating services with the ConfigStore system, based on the V2 implementation plan's trait-based design philosophy.

## Core Philosophy

**"Every component independently testable, every integration documented, every pattern reusable"**

The ConfigStore system provides a clean abstraction that allows services to:
- Load and validate configuration from multiple sources
- Test configuration logic without external dependencies
- Implement caching strategies
- Handle configuration changes gracefully

## ConfigStore Trait

The foundation of all configuration integration is the `ConfigStore` trait:

```rust
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

#[async_trait]
pub trait ConfigStore: Send + Sync {
    async fn get(&self, path: &str) -> Result<ConfigValue, ConfigError>;
    async fn set(&self, path: &str, value: ConfigValue) -> Result<(), ConfigError>;
    async fn get_tree(&self, prefix: &str) -> Result<ConfigTree, ConfigError>;
    async fn watch(&self, path: &str) -> Result<ConfigWatcher, ConfigError>;
}
```

## ServiceConfig Pattern

The `ServiceConfig` pattern is the standard way for services to integrate with ConfigStore:

```rust
use std::sync::Arc;
use serde::de::DeserializeOwned;
use tokio::sync::RwLock;

/// Standard pattern for service configuration integration
pub struct ServiceConfig<T> {
    store: Arc<dyn ConfigStore>,
    path: String,
    cache: RwLock<Option<T>>,
    validator: Box<dyn Validator<T>>,
    refresh_interval: Duration,
}

impl<T: DeserializeOwned + Clone + Send + Sync> ServiceConfig<T> {
    /// Create a new service configuration
    pub fn new(
        store: Arc<dyn ConfigStore>,
        path: impl Into<String>,
        validator: Box<dyn Validator<T>>,
    ) -> Self {
        Self {
            store,
            path: path.into(),
            cache: RwLock::new(None),
            validator,
            refresh_interval: Duration::from_secs(60),
        }
    }

    /// Load configuration with validation and caching
    pub async fn load(&self) -> Result<T, ConfigError> {
        // Fetch raw configuration from store
        let raw_value = self.store.get(&self.path).await
            .map_err(|e| ConfigError::LoadFailed {
                path: self.path.clone(),
                source: Box::new(e),
            })?;

        // Deserialize to target type
        let config: T = serde_json::from_value(raw_value.into())
            .map_err(|e| ConfigError::DeserializationFailed {
                path: self.path.clone(),
                source: Box::new(e),
            })?;

        // Validate configuration
        self.validator.validate(&config)
            .map_err(|e| ConfigError::ValidationFailed {
                path: self.path.clone(),
                errors: e,
            })?;

        // Update cache
        {
            let mut cache = self.cache.write().await;
            *cache = Some(config.clone());
        }

        Ok(config)
    }

    /// Get cached configuration or load if not cached
    pub async fn get(&self) -> Result<T, ConfigError> {
        {
            let cache = self.cache.read().await;
            if let Some(ref config) = *cache {
                return Ok(config.clone());
            }
        }

        self.load().await
    }

    /// Refresh configuration from store
    pub async fn refresh(&self) -> Result<bool, ConfigError> {
        let new_config = self.load().await?;
        
        {
            let cache = self.cache.read().await;
            if let Some(ref cached) = *cache {
                // Compare configurations to detect changes
                let current_json = serde_json::to_value(cached).unwrap();
                let new_json = serde_json::to_value(&new_config).unwrap();
                
                if current_json == new_json {
                    return Ok(false); // No changes
                }
            }
        }

        Ok(true) // Configuration changed
    }

    /// Watch for configuration changes
    pub async fn watch_changes(&self) -> Result<ConfigWatcher, ConfigError> {
        self.store.watch(&self.path).await
    }
}
```

## Caching Strategies

### 1. Simple Cache (Default)
The basic caching strategy stores the last loaded configuration in memory:

```rust
impl<T> ServiceConfig<T> {
    /// Enable simple caching (default behavior)
    pub fn with_simple_cache(mut self) -> Self {
        // Already implemented in the basic ServiceConfig
        self
    }
}
```

### 2. TTL Cache
For configurations that change frequently, implement TTL-based caching:

```rust
pub struct TtlServiceConfig<T> {
    inner: ServiceConfig<T>,
    ttl: Duration,
    last_loaded: RwLock<Option<Instant>>,
}

impl<T: DeserializeOwned + Clone + Send + Sync> TtlServiceConfig<T> {
    pub async fn get(&self) -> Result<T, ConfigError> {
        {
            let last_loaded = self.last_loaded.read().await;
            if let Some(loaded_time) = *last_loaded {
                if loaded_time.elapsed() < self.ttl {
                    return self.inner.get().await;
                }
            }
        }

        // TTL expired, force reload
        let config = self.inner.load().await?;
        
        {
            let mut last_loaded = self.last_loaded.write().await;
            *last_loaded = Some(Instant::now());
        }

        Ok(config)
    }
}
```

### 3. Event-Driven Cache
For real-time configuration updates, use event-driven caching:

```rust
pub struct EventDrivenServiceConfig<T> {
    inner: ServiceConfig<T>,
    event_bus: Arc<dyn EventBus>,
    subscription: Option<EventSubscription>,
}

impl<T: DeserializeOwned + Clone + Send + Sync> EventDrivenServiceConfig<T> {
    pub async fn start_watching(&mut self) -> Result<(), ConfigError> {
        let mut watcher = self.inner.watch_changes().await?;
        
        // Spawn background task to handle config changes
        let inner = self.inner.clone();
        tokio::spawn(async move {
            while let Some(change) = watcher.next().await {
                if let Err(e) = inner.refresh().await {
                    log::error!("Failed to refresh config on change: {}", e);
                }
            }
        });

        Ok(())
    }
}
```

## Error Handling Patterns

### Structured Error Types
Define comprehensive error types for configuration operations:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to load configuration from path '{path}': {source}")]
    LoadFailed {
        path: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Failed to deserialize configuration at path '{path}': {source}")]
    DeserializationFailed {
        path: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Configuration validation failed for path '{path}': {errors:?}")]
    ValidationFailed {
        path: String,
        errors: Vec<ValidationError>,
    },

    #[error("Configuration not found at path '{path}'")]
    NotFound { path: String },

    #[error("Store connection failed: {source}")]
    ConnectionFailed {
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}
```

### Graceful Degradation
Implement fallback strategies for configuration failures:

```rust
pub struct ResilientServiceConfig<T> {
    primary: ServiceConfig<T>,
    fallback: Option<T>,
    degraded_mode: bool,
}

impl<T: DeserializeOwned + Clone + Send + Sync> ResilientServiceConfig<T> {
    pub async fn get_or_fallback(&self) -> Result<T, ConfigError> {
        match self.primary.get().await {
            Ok(config) => Ok(config),
            Err(e) => {
                log::warn!("Primary config failed, using fallback: {}", e);
                
                if let Some(ref fallback) = self.fallback {
                    Ok(fallback.clone())
                } else {
                    Err(e)
                }
            }
        }
    }
}
```

## Configuration Validation

### Validation Trait
Define a standard validation interface:

```rust
pub trait Validator<T>: Send + Sync {
    fn validate(&self, config: &T) -> Result<(), Vec<ValidationError>>;
}

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
    pub code: String,
}
```

### Built-in Validators
Provide common validation implementations:

```rust
// Range validator for numeric fields
pub struct RangeValidator {
    pub min: f64,
    pub max: f64,
    pub field: String,
}

impl<T> Validator<T> for RangeValidator 
where 
    T: serde::Serialize,
{
    fn validate(&self, config: &T) -> Result<(), Vec<ValidationError>> {
        let value = serde_json::to_value(config).unwrap();
        let field_value = value.get(&self.field)
            .and_then(|v| v.as_f64())
            .ok_or_else(|| vec![ValidationError {
                field: self.field.clone(),
                message: "Field not found or not numeric".to_string(),
                code: "FIELD_MISSING_OR_INVALID".to_string(),
            }])?;

        if field_value < self.min || field_value > self.max {
            return Err(vec![ValidationError {
                field: self.field.clone(),
                message: format!("Value {} is outside range [{}, {}]", 
                    field_value, self.min, self.max),
                code: "VALUE_OUT_OF_RANGE".to_string(),
            }]);
        }

        Ok(())
    }
}

// Composite validator for multiple validations
pub struct CompositeValidator<T> {
    validators: Vec<Box<dyn Validator<T>>>,
}

impl<T> Validator<T> for CompositeValidator<T> {
    fn validate(&self, config: &T) -> Result<(), Vec<ValidationError>> {
        let mut all_errors = Vec::new();
        
        for validator in &self.validators {
            if let Err(mut errors) = validator.validate(config) {
                all_errors.append(&mut errors);
            }
        }

        if all_errors.is_empty() {
            Ok(())
        } else {
            Err(all_errors)
        }
    }
}
```

## Service Integration Example

Here's a complete example of how a service should integrate with ConfigStore:

```rust
use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingServiceConfig {
    pub max_position_size: f64,
    pub risk_threshold: f64,
    pub trading_hours: TradingHours,
    pub api_endpoints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingHours {
    pub start: String,
    pub end: String,
    pub timezone: String,
}

pub struct TradingService {
    config: ServiceConfig<TradingServiceConfig>,
    // ... other fields
}

impl TradingService {
    pub async fn new(config_store: Arc<dyn ConfigStore>) -> Result<Self, ConfigError> {
        // Create validator
        let validator = CompositeValidator::new()
            .add(RangeValidator::new("max_position_size", 0.0, 1000000.0))
            .add(RangeValidator::new("risk_threshold", 0.0, 1.0));

        // Create service config
        let config = ServiceConfig::new(
            config_store,
            "trading_service",
            Box::new(validator),
        );

        // Load initial configuration
        config.load().await?;

        Ok(Self { config })
    }

    pub async fn get_max_position_size(&self) -> Result<f64, ConfigError> {
        let config = self.config.get().await?;
        Ok(config.max_position_size)
    }

    pub async fn refresh_config(&self) -> Result<bool, ConfigError> {
        self.config.refresh().await
    }
}
```

## Testing Patterns

### Mock ConfigStore for Unit Tests
```rust
pub struct MockConfigStore {
    data: Arc<RwLock<HashMap<String, ConfigValue>>>,
}

impl MockConfigStore {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn set_mock_data(&self, path: &str, value: ConfigValue) {
        let mut data = self.data.write().await;
        data.insert(path.to_string(), value);
    }
}

#[async_trait]
impl ConfigStore for MockConfigStore {
    async fn get(&self, path: &str) -> Result<ConfigValue, ConfigError> {
        let data = self.data.read().await;
        data.get(path)
            .cloned()
            .ok_or_else(|| ConfigError::NotFound { 
                path: path.to_string() 
            })
    }

    async fn set(&self, path: &str, value: ConfigValue) -> Result<(), ConfigError> {
        let mut data = self.data.write().await;
        data.insert(path.to_string(), value);
        Ok(())
    }

    async fn get_tree(&self, prefix: &str) -> Result<ConfigTree, ConfigError> {
        let data = self.data.read().await;
        let mut tree = ConfigTree::new();
        
        for (key, value) in data.iter() {
            if key.starts_with(prefix) {
                tree.insert(key.clone(), value.clone());
            }
        }
        
        Ok(tree)
    }

    async fn watch(&self, _path: &str) -> Result<ConfigWatcher, ConfigError> {
        // Return a mock watcher for testing
        Ok(ConfigWatcher::mock())
    }
}
```

### Test Example
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_trading_service_config_integration() {
        let mock_store = Arc::new(MockConfigStore::new());
        
        // Set up mock configuration
        let config_data = serde_json::json!({
            "max_position_size": 50000.0,
            "risk_threshold": 0.02,
            "trading_hours": {
                "start": "09:30",
                "end": "16:00",
                "timezone": "America/New_York"
            },
            "api_endpoints": ["https://api.example.com"]
        });
        
        mock_store.set_mock_data("trading_service", ConfigValue::Object(config_data.as_object().unwrap().clone())).await;

        // Test service initialization
        let service = TradingService::new(mock_store).await.unwrap();
        
        // Test configuration access
        let max_position = service.get_max_position_size().await.unwrap();
        assert_eq!(max_position, 50000.0);
    }

    #[tokio::test]
    async fn test_config_validation() {
        let mock_store = Arc::new(MockConfigStore::new());
        
        // Set up invalid configuration
        let invalid_config = serde_json::json!({
            "max_position_size": -1000.0, // Invalid: negative value
            "risk_threshold": 1.5, // Invalid: > 1.0
            "trading_hours": {
                "start": "09:30",
                "end": "16:00",
                "timezone": "America/New_York"
            },
            "api_endpoints": []
        });
        
        mock_store.set_mock_data("trading_service", ConfigValue::Object(invalid_config.as_object().unwrap().clone())).await;

        // Test that service creation fails with validation errors
        let result = TradingService::new(mock_store).await;
        assert!(result.is_err());
        
        if let Err(ConfigError::ValidationFailed { errors, .. }) = result {
            assert_eq!(errors.len(), 2); // Two validation errors
        } else {
            panic!("Expected ValidationFailed error");
        }
    }
}
```

## Best Practices

1. **Use Type-Safe Configuration**: Always define strongly-typed configuration structs
2. **Implement Comprehensive Validation**: Validate all configuration parameters
3. **Cache Appropriately**: Choose the right caching strategy for your use case
4. **Handle Errors Gracefully**: Implement fallback mechanisms for critical services
5. **Test Thoroughly**: Use mock implementations for unit testing
6. **Monitor Configuration**: Track configuration changes and validation failures
7. **Document Configuration Schema**: Maintain clear documentation of all configuration options

## Performance Considerations

- **Minimize Store Access**: Use caching to reduce calls to the configuration store
- **Batch Operations**: When possible, load related configurations together
- **Async Operations**: All configuration operations should be async to avoid blocking
- **Connection Pooling**: Reuse connections to the configuration store
- **Validation Efficiency**: Keep validators lightweight and fast

This pattern provides a robust foundation for configuration management that supports testing, validation, caching, and error handling while maintaining clean separation of concerns.