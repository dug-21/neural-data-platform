# Config-Store Implementation Tasks

## Task 1: Implement RedisConfigStore

### File: `config-store/src/stores/redis.rs`

```rust
use async_trait::async_trait;
use redis::{aio::ConnectionManager, Client, RedisError};
use serde_json::Value as JsonValue;
use std::time::Duration;
use crate::{ConfigError, ConfigStore, ConfigValue, Result};

pub struct RedisConfigStore {
    client: Client,
    conn: ConnectionManager,
    prefix: String,
    ttl: Option<Duration>,
}

impl RedisConfigStore {
    pub async fn new(url: &str) -> Result<Self> {
        let client = Client::open(url)
            .map_err(|e| ConfigError::ConnectionError(e.to_string()))?;
        let conn = ConnectionManager::new(client.clone()).await
            .map_err(|e| ConfigError::ConnectionError(e.to_string()))?;
        
        Ok(Self {
            client,
            conn,
            prefix: "config:".to_string(),
            ttl: None,
        })
    }
    
    pub fn builder() -> RedisConfigStoreBuilder {
        RedisConfigStoreBuilder::default()
    }
}

#[async_trait]
impl ConfigStore for RedisConfigStore {
    async fn get(&self, key: &str) -> Result<ConfigValue> {
        let redis_key = format!("{}{}", self.prefix, key);
        let value: String = redis::cmd("GET")
            .arg(&redis_key)
            .query_async(&mut self.conn.clone())
            .await
            .map_err(|e| ConfigError::NotFound(key.to_string()))?;
        
        let json_value: JsonValue = serde_json::from_str(&value)
            .map_err(|e| ConfigError::SerializationError(e.to_string()))?;
        
        Ok(ConfigValue::from_json(json_value))
    }
    
    async fn set(&mut self, key: &str, value: ConfigValue) -> Result<()> {
        let redis_key = format!("{}{}", self.prefix, key);
        let json_value = value.to_json();
        let serialized = serde_json::to_string(&json_value)
            .map_err(|e| ConfigError::SerializationError(e.to_string()))?;
        
        if let Some(ttl) = self.ttl {
            redis::cmd("SETEX")
                .arg(&redis_key)
                .arg(ttl.as_secs())
                .arg(serialized)
                .query_async(&mut self.conn.clone())
                .await
        } else {
            redis::cmd("SET")
                .arg(&redis_key)
                .arg(serialized)
                .query_async(&mut self.conn.clone())
                .await
        }.map_err(|e| ConfigError::StorageError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn delete(&mut self, key: &str) -> Result<()> {
        let redis_key = format!("{}{}", self.prefix, key);
        redis::cmd("DEL")
            .arg(&redis_key)
            .query_async(&mut self.conn.clone())
            .await
            .map_err(|e| ConfigError::StorageError(e.to_string()))?;
        Ok(())
    }
    
    async fn exists(&self, key: &str) -> Result<bool> {
        let redis_key = format!("{}{}", self.prefix, key);
        let exists: bool = redis::cmd("EXISTS")
            .arg(&redis_key)
            .query_async(&mut self.conn.clone())
            .await
            .map_err(|e| ConfigError::StorageError(e.to_string()))?;
        Ok(exists)
    }
    
    async fn list_keys(&self, prefix: Option<&str>) -> Result<Vec<String>> {
        let pattern = match prefix {
            Some(p) => format!("{}{}*", self.prefix, p),
            None => format!("{}*", self.prefix),
        };
        
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(&pattern)
            .query_async(&mut self.conn.clone())
            .await
            .map_err(|e| ConfigError::StorageError(e.to_string()))?;
        
        Ok(keys.into_iter()
            .map(|k| k.strip_prefix(&self.prefix).unwrap_or(&k).to_string())
            .collect())
    }
}
```

### Dependencies to add to `config-store/Cargo.toml`:
```toml
redis = { version = "0.25", features = ["tokio-comp", "connection-manager"] }
```

## Task 2: Implement FileConfigStore

### File: `config-store/src/stores/file.rs`

```rust
use async_trait::async_trait;
use notify::{Watcher, RecursiveMode, watcher};
use serde_json::Value as JsonValue;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use tokio::fs;
use tokio::sync::RwLock;
use crate::{ConfigError, ConfigStore, ConfigValue, Result};

pub struct FileConfigStore {
    base_path: PathBuf,
    cache: Arc<RwLock<HashMap<String, ConfigValue>>>,
    watch_enabled: bool,
    format: ConfigFormat,
}

#[derive(Clone, Copy)]
pub enum ConfigFormat {
    Json,
    Yaml,
    Toml,
}

impl FileConfigStore {
    pub async fn new<P: AsRef<Path>>(base_path: P) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        fs::create_dir_all(&base_path).await
            .map_err(|e| ConfigError::StorageError(e.to_string()))?;
        
        Ok(Self {
            base_path,
            cache: Arc::new(RwLock::new(HashMap::new())),
            watch_enabled: false,
            format: ConfigFormat::Json,
        })
    }
    
    pub fn builder() -> FileConfigStoreBuilder {
        FileConfigStoreBuilder::default()
    }
    
    fn key_to_path(&self, key: &str) -> PathBuf {
        let file_name = format!("{}.{}", key.replace('/', "_"), 
            match self.format {
                ConfigFormat::Json => "json",
                ConfigFormat::Yaml => "yaml",
                ConfigFormat::Toml => "toml",
            });
        self.base_path.join(file_name)
    }
    
    async fn load_from_file(&self, path: &Path) -> Result<ConfigValue> {
        let content = fs::read_to_string(path).await
            .map_err(|e| ConfigError::NotFound(path.display().to_string()))?;
        
        let value = match self.format {
            ConfigFormat::Json => {
                serde_json::from_str(&content)
                    .map_err(|e| ConfigError::SerializationError(e.to_string()))?
            },
            ConfigFormat::Yaml => {
                serde_yaml::from_str(&content)
                    .map_err(|e| ConfigError::SerializationError(e.to_string()))?
            },
            ConfigFormat::Toml => {
                toml::from_str(&content)
                    .map_err(|e| ConfigError::SerializationError(e.to_string()))?
            },
        };
        
        Ok(ConfigValue::from_json(value))
    }
}

#[async_trait]
impl ConfigStore for FileConfigStore {
    async fn get(&self, key: &str) -> Result<ConfigValue> {
        // Check cache first
        if let Some(value) = self.cache.read().await.get(key) {
            return Ok(value.clone());
        }
        
        let path = self.key_to_path(key);
        let value = self.load_from_file(&path).await?;
        
        // Update cache
        self.cache.write().await.insert(key.to_string(), value.clone());
        
        Ok(value)
    }
    
    async fn set(&mut self, key: &str, value: ConfigValue) -> Result<()> {
        let path = self.key_to_path(key);
        let json_value = value.to_json();
        
        let content = match self.format {
            ConfigFormat::Json => serde_json::to_string_pretty(&json_value),
            ConfigFormat::Yaml => serde_yaml::to_string(&json_value),
            ConfigFormat::Toml => toml::to_string_pretty(&json_value),
        }.map_err(|e| ConfigError::SerializationError(e.to_string()))?;
        
        fs::write(&path, content).await
            .map_err(|e| ConfigError::StorageError(e.to_string()))?;
        
        // Update cache
        self.cache.write().await.insert(key.to_string(), value);
        
        Ok(())
    }
    
    async fn delete(&mut self, key: &str) -> Result<()> {
        let path = self.key_to_path(key);
        fs::remove_file(&path).await
            .map_err(|e| ConfigError::StorageError(e.to_string()))?;
        
        // Remove from cache
        self.cache.write().await.remove(key);
        
        Ok(())
    }
    
    async fn exists(&self, key: &str) -> Result<bool> {
        let path = self.key_to_path(key);
        Ok(path.exists())
    }
    
    async fn list_keys(&self, prefix: Option<&str>) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        let mut entries = fs::read_dir(&self.base_path).await
            .map_err(|e| ConfigError::StorageError(e.to_string()))?;
        
        while let Some(entry) = entries.next_entry().await
            .map_err(|e| ConfigError::StorageError(e.to_string()))? {
            
            if let Some(name) = entry.file_name().to_str() {
                let key = name.split('.').next().unwrap_or(name)
                    .replace('_', "/");
                
                if let Some(p) = prefix {
                    if key.starts_with(p) {
                        keys.push(key);
                    }
                } else {
                    keys.push(key);
                }
            }
        }
        
        Ok(keys)
    }
}
```

### Dependencies to add:
```toml
notify = "6.1"
serde_yaml = "0.9"
toml = "0.8"
```

## Task 3: Implement ServiceConfig Pattern

### File: `config-store/src/service_config.rs`

```rust
use std::sync::Arc;
use std::marker::PhantomData;
use serde::{Deserialize, Serialize};
use crate::{ConfigStore, ConfigError, Result};

pub trait Validator<T>: Send + Sync {
    fn validate(&self, config: &T) -> Result<()>;
}

pub struct ServiceConfig<T> {
    store: Arc<dyn ConfigStore>,
    path: String,
    validator: Box<dyn Validator<T>>,
    cache: Option<CacheStrategy>,
    _phantom: PhantomData<T>,
}

pub enum CacheStrategy {
    Simple,
    TTL(Duration),
    EventDriven(Arc<dyn EventBus>),
}

impl<T> ServiceConfig<T> 
where
    T: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static,
{
    pub fn new(
        store: Arc<dyn ConfigStore>,
        path: impl Into<String>,
        validator: Box<dyn Validator<T>>,
    ) -> Self {
        Self {
            store,
            path: path.into(),
            validator,
            cache: Some(CacheStrategy::Simple),
            _phantom: PhantomData,
        }
    }
    
    pub fn with_ttl_cache(mut self, ttl: Duration) -> Self {
        self.cache = Some(CacheStrategy::TTL(ttl));
        self
    }
    
    pub fn with_event_driven_cache(mut self, event_bus: Arc<dyn EventBus>) -> Self {
        self.cache = Some(CacheStrategy::EventDriven(event_bus));
        self
    }
    
    pub async fn load(&self) -> Result<T> {
        let value = self.store.get(&self.path).await?;
        let config: T = value.try_into()
            .map_err(|e| ConfigError::SerializationError(e.to_string()))?;
        
        self.validator.validate(&config)?;
        
        Ok(config)
    }
    
    pub async fn save(&self, config: &T) -> Result<()> {
        self.validator.validate(config)?;
        
        let value = ConfigValue::from_serializable(config)?;
        self.store.clone().set(&self.path, value).await?;
        
        Ok(())
    }
    
    pub async fn watch<F>(&self, callback: F) -> Result<()>
    where
        F: Fn(T) + Send + Sync + 'static,
    {
        // Implementation for watching configuration changes
        // Would integrate with FileConfigStore's notify watcher
        // or Redis pub/sub for RedisConfigStore
        todo!("Implement watch functionality")
    }
}
```

## Task 4: Add Validation Framework

### File: `config-store/src/validators/mod.rs`

```rust
use crate::{Result, ConfigError};
use super::Validator;

pub struct RangeValidator<T> {
    min: T,
    max: T,
}

impl<T: PartialOrd + Display> Validator<T> for RangeValidator<T> {
    fn validate(&self, value: &T) -> Result<()> {
        if value < &self.min || value > &self.max {
            return Err(ConfigError::ValidationError(
                format!("Value {} is out of range [{}, {}]", value, self.min, self.max)
            ));
        }
        Ok(())
    }
}

pub struct CompositeValidator<T> {
    validators: Vec<Box<dyn Validator<T>>>,
}

impl<T> Validator<T> for CompositeValidator<T> {
    fn validate(&self, value: &T) -> Result<()> {
        for validator in &self.validators {
            validator.validate(value)?;
        }
        Ok(())
    }
}

// Example: Trading Config Validator
pub struct TradingConfigValidator;

impl Validator<TradingConfig> for TradingConfigValidator {
    fn validate(&self, config: &TradingConfig) -> Result<()> {
        if config.max_position_size <= 0.0 {
            return Err(ConfigError::ValidationError(
                "max_position_size must be positive".to_string()
            ));
        }
        
        if config.risk_tolerance < 0.0 || config.risk_tolerance > 1.0 {
            return Err(ConfigError::ValidationError(
                "risk_tolerance must be between 0 and 1".to_string()
            ));
        }
        
        Ok(())
    }
}
```

## Task 5: Integration Tests

### File: `config-store/tests/integration_tests.rs`

```rust
#[tokio::test]
async fn test_redis_store_integration() {
    let store = RedisConfigStore::new("redis://localhost:6379").await.unwrap();
    test_store_operations(store).await;
}

#[tokio::test]
async fn test_file_store_integration() {
    let temp_dir = tempfile::tempdir().unwrap();
    let store = FileConfigStore::new(temp_dir.path()).await.unwrap();
    test_store_operations(store).await;
}

#[tokio::test]
async fn test_service_config_with_validation() {
    let store = Arc::new(InMemoryConfigStore::new());
    let validator = Box::new(TradingConfigValidator);
    let config_service = ServiceConfig::new(store, "trading", validator);
    
    let config = TradingConfig {
        max_position_size: 10000.0,
        risk_tolerance: 0.02,
        enable_paper_trading: true,
    };
    
    config_service.save(&config).await.unwrap();
    let loaded = config_service.load().await.unwrap();
    
    assert_eq!(loaded.max_position_size, 10000.0);
}

async fn test_store_operations<S: ConfigStore>(mut store: S) {
    // Test set and get
    store.set("test/key", ConfigValue::String("value".to_string())).await.unwrap();
    let value = store.get("test/key").await.unwrap();
    assert_eq!(value, ConfigValue::String("value".to_string()));
    
    // Test nested structures
    let nested = ConfigValue::Object(HashMap::from([
        ("host".to_string(), ConfigValue::String("localhost".to_string())),
        ("port".to_string(), ConfigValue::Number(5432.into())),
    ]));
    store.set("database", nested.clone()).await.unwrap();
    let loaded = store.get("database").await.unwrap();
    assert_eq!(loaded, nested);
    
    // Test list operations
    let keys = store.list_keys(Some("test/")).await.unwrap();
    assert!(keys.contains(&"test/key".to_string()));
    
    // Test delete
    store.delete("test/key").await.unwrap();
    assert!(!store.exists("test/key").await.unwrap());
}
```

## Implementation Priority

### Week 1 (Core Infrastructure)
- [ ] Day 1-2: Implement RedisConfigStore with basic operations
- [ ] Day 3-4: Implement FileConfigStore with JSON support
- [ ] Day 5: Add ServiceConfig<T> pattern

### Week 2 (Advanced Features)
- [ ] Day 1-2: Add validation framework
- [ ] Day 3: Implement caching strategies
- [ ] Day 4-5: Add hot-reloading for FileConfigStore

### Week 3 (Integration)
- [ ] Day 1-2: Restore Python gRPC client
- [ ] Day 3: Add watch/subscription mechanisms
- [ ] Day 4-5: Complete integration tests

### Week 4 (Polish)
- [ ] Day 1-2: Performance optimization
- [ ] Day 3: Documentation
- [ ] Day 4-5: Migration tooling

## Testing Strategy

1. **Unit Tests**: Each store implementation tested independently
2. **Integration Tests**: Test all stores with common test suite
3. **Performance Tests**: Benchmark Redis vs File vs Memory
4. **Migration Tests**: Ensure backward compatibility

## Migration Path

1. Keep InMemoryConfigStore as default
2. Add feature flags for Redis/File stores
3. Gradual rollout by service
4. Full migration after validation

## Success Criteria

- [ ] All original specification features implemented
- [ ] Performance meets or exceeds requirements
- [ ] Zero downtime migration path
- [ ] 100% backward compatibility
- [ ] Comprehensive test coverage (>80%)