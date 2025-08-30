# Config-Store Complete Implementation Plan

## Executive Summary

This document provides a comprehensive, quality-focused implementation plan to restore config-store to its original specification with enhanced security features. This plan emphasizes **NO SHORTCUTS** - every component will be implemented to production standards with full testing and documentation.

## Current State Analysis

### What We Have (54% Complete)
1. **Core Types & Traits** ✅
   - `ConfigValue`, `ConfigError`, `ConfigTree`, `ConfigNode`
   - `ConfigStore` trait definition
   - `InMemoryConfigStore` with nested path support

2. **Security Features** ✅ (Added post-spec, MUST PRESERVE)
   - Rate limiting (100 req/min)
   - Input sanitization (XSS prevention)
   - Blocklist validation
   - Secure JSON parsing
   - Safe configuration loader

3. **Configuration Types** ✅ (2,034 lines restored)
   - `EnhancedNeuralConfig` (844 lines)
   - `DatabaseConfig`, `RedisConfig`
   - `MonitoringConfig`, `SecurityConfig`
   - `FeatureFlags`
   - `PlatformConfig` with builder pattern

4. **Proto Definition** ✅
   - `/proto/config_store.proto` complete
   - Shared `build.rs` for compilation

### What's Missing (46% to Implement)

## 1. RedisConfigStore Implementation

### Requirements from Specification
- **Connection**: Redis client with connection pooling (r2d2)
- **Performance**: < 10ms read latency, < 50ms write latency
- **Throughput**: 10,000 reads/sec, 1,000 writes/sec
- **Caching**: Local cache with 60s TTL, > 90% hit rate
- **Transactions**: Atomic operations with rollback
- **Versioning**: Track last 10 versions per key
- **Inheritance**: Resolve parent configurations

### Detailed Implementation

```rust
// config-store/src/stores/redis_store.rs

use redis::{Client, aio::ConnectionManager};
use r2d2_redis::RedisConnectionManager;
use dashmap::DashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

pub struct RedisConfigStore {
    // Connection Management
    client: Client,
    connection_pool: r2d2::Pool<RedisConnectionManager>,
    async_connection: Arc<RwLock<ConnectionManager>>,
    
    // Caching Layer
    cache: Arc<DashMap<String, CachedValue>>,
    cache_ttl: Duration,
    
    // Configuration
    key_prefix: String,
    max_versions: u32,  // Default: 10
    
    // Monitoring
    metrics: Arc<StoreMetrics>,
}

struct CachedValue {
    value: ConfigValue,
    inserted_at: Instant,
    version: u32,
}

struct StoreMetrics {
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    read_latency_ms: Histogram,
    write_latency_ms: Histogram,
}

impl RedisConfigStore {
    pub async fn new(url: &str, config: RedisStoreConfig) -> Result<Self, ConfigError> {
        // Initialize connection pool
        let manager = RedisConnectionManager::new(url)?;
        let pool = r2d2::Pool::builder()
            .max_size(config.pool_size)
            .min_idle(Some(config.min_idle))
            .connection_timeout(Duration::from_secs(5))
            .build(manager)?;
            
        // Test connection
        let mut conn = pool.get()?;
        redis::cmd("PING").query::<String>(&mut *conn)?;
        
        // Initialize async connection
        let client = Client::open(url)?;
        let async_conn = ConnectionManager::new(client.clone()).await?;
        
        Ok(RedisConfigStore {
            client,
            connection_pool: pool,
            async_connection: Arc::new(RwLock::new(async_conn)),
            cache: Arc::new(DashMap::new()),
            cache_ttl: Duration::from_secs(config.cache_ttl_secs),
            key_prefix: config.key_prefix,
            max_versions: config.max_versions,
            metrics: Arc::new(StoreMetrics::default()),
        })
    }
    
    async fn get_with_cache(&self, path: &str) -> Result<ConfigValue, ConfigError> {
        let cache_key = self.format_key(path);
        
        // Check cache first
        if let Some(cached) = self.cache.get(&cache_key) {
            if cached.inserted_at.elapsed() < self.cache_ttl {
                self.metrics.cache_hits.fetch_add(1, Ordering::Relaxed);
                return Ok(cached.value.clone());
            }
            // Remove expired entry
            self.cache.remove(&cache_key);
        }
        
        self.metrics.cache_misses.fetch_add(1, Ordering::Relaxed);
        
        // Fetch from Redis
        let start = Instant::now();
        let value = self.fetch_from_redis(path).await?;
        let latency = start.elapsed().as_millis() as u64;
        self.metrics.read_latency_ms.record(latency);
        
        // Update cache
        self.cache.insert(cache_key, CachedValue {
            value: value.clone(),
            inserted_at: Instant::now(),
            version: self.get_current_version(path).await?,
        });
        
        Ok(value)
    }
    
    async fn set_with_versioning(&self, path: &str, value: ConfigValue) -> Result<(), ConfigError> {
        let key = self.format_key(path);
        let version_key = self.format_version_key(path);
        
        // Start transaction
        let mut conn = self.async_connection.write().await;
        let mut pipe = redis::pipe();
        
        pipe.atomic();
        
        // Get current version
        let current_version: Option<u32> = redis::cmd("GET")
            .arg(&version_key)
            .query_async(&mut *conn)
            .await
            .unwrap_or(None);
            
        let new_version = current_version.unwrap_or(0) + 1;
        
        // Archive current value if exists
        if let Some(current) = self.get_current_value(path).await? {
            let history_key = self.format_history_key(path, current_version.unwrap());
            pipe.set_ex(history_key, serde_json::to_string(&current)?, 30 * 24 * 3600);
            
            // Maintain version limit
            if current_version.unwrap() >= self.max_versions {
                let old_key = self.format_history_key(path, current_version.unwrap() - self.max_versions);
                pipe.del(old_key);
            }
        }
        
        // Set new value with metadata
        let node = ConfigNode {
            path: path.to_string(),
            value: value.clone(),
            version: new_version,
            metadata: ConfigMetadata {
                updated_at: SystemTime::now(),
                updated_by: self.get_current_user(),
            },
        };
        
        pipe.set(&key, serde_json::to_string(&node)?);
        pipe.set(&version_key, new_version);
        
        // Publish change event
        pipe.publish(format!("config:changes:{}", path), serde_json::to_string(&node)?);
        
        // Execute transaction
        let start = Instant::now();
        pipe.query_async(&mut *conn).await?;
        let latency = start.elapsed().as_millis() as u64;
        self.metrics.write_latency_ms.record(latency);
        
        // Invalidate cache
        self.cache.remove(&key);
        
        Ok(())
    }
    
    async fn handle_inheritance(&self, path: &str, value: ConfigValue) -> Result<ConfigValue, ConfigError> {
        if !value.has_inheritance() {
            return Ok(value);
        }
        
        let mut merged = ConfigValue::Object(BTreeMap::new());
        
        // Resolve parent configs recursively
        for parent_path in value.get_inheritance_paths() {
            let parent = self.get(parent_path).await?;
            merged = self.merge_configs(merged, parent);
        }
        
        // Apply current config as override
        merged = self.merge_configs(merged, value);
        
        Ok(merged)
    }
}

#[async_trait]
impl ConfigStore for RedisConfigStore {
    async fn get(&self, path: &str) -> Result<ConfigValue, ConfigError> {
        validate_path(path)?;
        let value = self.get_with_cache(path).await?;
        self.handle_inheritance(path, value).await
    }
    
    async fn set(&self, path: &str, value: ConfigValue) -> Result<(), ConfigError> {
        validate_path(path)?;
        
        // Validate against schema if exists
        if let Some(schema) = self.get_schema(path).await? {
            validate_against_schema(&value, &schema)?;
        }
        
        self.set_with_versioning(path, value).await
    }
    
    async fn delete(&self, path: &str) -> Result<(), ConfigError> {
        validate_path(path)?;
        
        // Archive before deletion
        if let Ok(current) = self.get(path).await {
            self.archive_for_deletion(path, current).await?;
        }
        
        let key = self.format_key(path);
        let mut conn = self.async_connection.write().await;
        redis::cmd("DEL").arg(&key).query_async(&mut *conn).await?;
        
        // Invalidate cache
        self.cache.remove(&key);
        
        Ok(())
    }
    
    async fn get_tree(&self, prefix: &str) -> Result<ConfigTree, ConfigError> {
        validate_path(prefix)?;
        
        let pattern = format!("{}:{}*", self.key_prefix, prefix);
        let mut conn = self.async_connection.write().await;
        
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(pattern)
            .query_async(&mut *conn)
            .await?;
            
        let mut tree = ConfigTree::new();
        
        for key in keys {
            let path = key.strip_prefix(&format!("{}:", self.key_prefix))
                .ok_or(ConfigError::InvalidPath("Invalid key format"))?;
                
            let value = self.get(path).await?;
            tree.insert(path.to_string(), value);
        }
        
        Ok(tree)
    }
    
    async fn get_version(&self, path: &str, version: u32) -> Result<ConfigValue, ConfigError> {
        validate_path(path)?;
        
        let history_key = self.format_history_key(path, version);
        let mut conn = self.async_connection.write().await;
        
        let data: Option<String> = redis::cmd("GET")
            .arg(history_key)
            .query_async(&mut *conn)
            .await?;
            
        match data {
            Some(json) => Ok(serde_json::from_str(&json)?),
            None => Err(ConfigError::VersionNotFound(path.to_string(), version)),
        }
    }
    
    async fn get_history(&self, path: &str) -> Result<Vec<ConfigVersion>, ConfigError> {
        validate_path(path)?;
        
        let current_version = self.get_current_version(path).await?;
        let mut history = Vec::new();
        
        // Get current value
        if let Ok(current) = self.get(path).await {
            history.push(ConfigVersion {
                version: current_version,
                value: current,
                timestamp: SystemTime::now(),
            });
        }
        
        // Get historical versions
        let start_version = current_version.saturating_sub(self.max_versions - 1);
        for version in start_version..current_version {
            if let Ok(value) = self.get_version(path, version).await {
                history.push(ConfigVersion {
                    version,
                    value,
                    timestamp: self.get_version_timestamp(path, version).await?,
                });
            }
        }
        
        history.sort_by_key(|v| v.version);
        Ok(history)
    }
    
    async fn transaction<F, Fut>(&self, f: F) -> Result<(), ConfigError>
    where
        F: FnOnce(&mut ConfigTransaction) -> Fut,
        Fut: Future<Output = Result<(), ConfigError>>,
    {
        let mut tx = ConfigTransaction::new(self.clone());
        
        match f(&mut tx).await {
            Ok(()) => tx.commit().await,
            Err(e) => {
                tx.rollback().await;
                Err(e)
            }
        }
    }
}
```

## 2. FileConfigStore Implementation

### Requirements from Specification
- File-based storage for development/testing
- Hot-reload capability with file watching
- YAML/JSON support
- Directory-based hierarchy

### Detailed Implementation

```rust
// config-store/src/stores/file_store.rs

use notify::{Watcher, RecursiveMode, watcher};
use std::sync::mpsc::channel;
use tokio::sync::broadcast;

pub struct FileConfigStore {
    base_path: PathBuf,
    format: FileFormat,
    cache: Arc<RwLock<HashMap<String, ConfigValue>>>,
    watcher: Option<notify::RecommendedWatcher>,
    change_tx: broadcast::Sender<ConfigChangeEvent>,
}

impl FileConfigStore {
    pub fn new(base_path: impl AsRef<Path>, config: FileStoreConfig) -> Result<Self, ConfigError> {
        let base_path = base_path.as_ref().to_path_buf();
        
        if !base_path.exists() {
            std::fs::create_dir_all(&base_path)?;
        }
        
        let (change_tx, _) = broadcast::channel(100);
        let cache = Arc::new(RwLock::new(HashMap::new()));
        
        // Setup file watcher for hot-reload
        let watcher = if config.hot_reload {
            let (tx, rx) = channel();
            let mut watcher = watcher(tx, Duration::from_secs(1))?;
            watcher.watch(&base_path, RecursiveMode::Recursive)?;
            
            // Spawn handler for file changes
            let cache_clone = cache.clone();
            let change_tx_clone = change_tx.clone();
            tokio::spawn(async move {
                while let Ok(event) = rx.recv() {
                    Self::handle_file_change(event, &cache_clone, &change_tx_clone).await;
                }
            });
            
            Some(watcher)
        } else {
            None
        };
        
        // Initial load
        let mut store = FileConfigStore {
            base_path,
            format: config.format,
            cache,
            watcher,
            change_tx,
        };
        
        store.reload_all()?;
        
        Ok(store)
    }
    
    fn path_to_file(&self, path: &str) -> PathBuf {
        let clean_path = path.trim_start_matches('/');
        let file_name = format!("{}.{}", 
            clean_path.replace('/', "_"),
            self.format.extension()
        );
        self.base_path.join(file_name)
    }
    
    async fn reload_all(&mut self) -> Result<(), ConfigError> {
        let mut cache = self.cache.write().await;
        cache.clear();
        
        for entry in std::fs::read_dir(&self.base_path)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() && path.extension() == Some(self.format.extension().as_ref()) {
                let config_path = self.file_to_path(&path)?;
                let value = self.load_file(&path)?;
                cache.insert(config_path, value);
            }
        }
        
        Ok(())
    }
    
    async fn handle_file_change(
        event: notify::DebouncedEvent,
        cache: &Arc<RwLock<HashMap<String, ConfigValue>>>,
        change_tx: &broadcast::Sender<ConfigChangeEvent>,
    ) {
        match event {
            notify::DebouncedEvent::Write(path) |
            notify::DebouncedEvent::Create(path) => {
                if let Ok(value) = Self::load_file(&path) {
                    let config_path = Self::file_to_path(&path).unwrap();
                    let mut cache = cache.write().await;
                    
                    let old_value = cache.get(&config_path).cloned();
                    cache.insert(config_path.clone(), value.clone());
                    
                    let _ = change_tx.send(ConfigChangeEvent {
                        path: config_path,
                        old_value,
                        new_value: Some(value),
                        change_type: if old_value.is_some() { 
                            ChangeType::Updated 
                        } else { 
                            ChangeType::Created 
                        },
                        timestamp: SystemTime::now(),
                    });
                }
            },
            notify::DebouncedEvent::Remove(path) => {
                let config_path = Self::file_to_path(&path).unwrap();
                let mut cache = cache.write().await;
                
                if let Some(old_value) = cache.remove(&config_path) {
                    let _ = change_tx.send(ConfigChangeEvent {
                        path: config_path,
                        old_value: Some(old_value),
                        new_value: None,
                        change_type: ChangeType::Deleted,
                        timestamp: SystemTime::now(),
                    });
                }
            },
            _ => {}
        }
    }
}

#[async_trait]
impl ConfigStore for FileConfigStore {
    async fn get(&self, path: &str) -> Result<ConfigValue, ConfigError> {
        validate_path(path)?;
        
        let cache = self.cache.read().await;
        cache.get(path)
            .cloned()
            .ok_or_else(|| ConfigError::NotFound(path.to_string()))
    }
    
    async fn set(&self, path: &str, value: ConfigValue) -> Result<(), ConfigError> {
        validate_path(path)?;
        
        let file_path = self.path_to_file(path);
        
        // Ensure directory exists
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        // Write to file
        let content = match self.format {
            FileFormat::Json => serde_json::to_string_pretty(&value)?,
            FileFormat::Yaml => serde_yaml::to_string(&value)?,
        };
        
        std::fs::write(&file_path, content)?;
        
        // Update cache
        let mut cache = self.cache.write().await;
        cache.insert(path.to_string(), value);
        
        Ok(())
    }
}
```

## 3. gRPC Service Implementation

### Requirements
- Complete ConfigStoreService implementation
- Streaming support for WatchConfig
- Health checks
- Proper error handling

### Implementation Fix

```rust
// config-store/src/bin/config-store-server.rs

// Proper imports for proto types
use neural_platform::config::*;

#[async_trait]
impl ConfigStoreService for ConfigStoreServiceImpl {
    async fn get_config(
        &self,
        request: Request<GetConfigRequest>,
    ) -> Result<Response<GetConfigResponse>, Status> {
        let req = request.into_inner();
        
        let path = format!("{}/{}", req.namespace_path, req.key);
        
        match self.store.get(&path).await {
            Ok(value) => {
                let response = GetConfigResponse {
                    success: true,
                    namespace_path: req.namespace_path,
                    key: req.key,
                    version: "1".to_string(),
                    value: Some(convert_to_proto_value(value)),
                    metadata: if req.include_metadata {
                        Some(self.get_metadata(&path).await?)
                    } else {
                        None
                    },
                    error_message: String::new(),
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                Ok(Response::new(GetConfigResponse {
                    success: false,
                    error_message: e.to_string(),
                    ..Default::default()
                }))
            }
        }
    }
    
    type WatchConfigStream = Pin<Box<dyn Stream<Item = Result<ConfigChangeEvent, Status>> + Send>>;
    
    async fn watch_config(
        &self,
        request: Request<WatchConfigRequest>,
    ) -> Result<Response<Self::WatchConfigStream>, Status> {
        let req = request.into_inner();
        let mut rx = self.change_tx.subscribe();
        
        let stream = async_stream::stream! {
            // Send initial values if requested
            if req.include_initial_values {
                for key in &req.keys {
                    let path = format!("{}/{}", req.namespace_path, key);
                    if let Ok(value) = self.store.get(&path).await {
                        yield Ok(ConfigChangeEvent {
                            namespace_path: req.namespace_path.clone(),
                            key: key.clone(),
                            change_type: ChangeType::Created as i32,
                            old_value: None,
                            new_value: Some(convert_to_proto_value(value)),
                            timestamp: Some(system_time_to_timestamp(SystemTime::now())),
                            change_reason: "Initial value".to_string(),
                            changed_by: "system".to_string(),
                            version: "1".to_string(),
                        });
                    }
                }
            }
            
            // Stream changes
            while let Ok(event) = rx.recv().await {
                // Filter by namespace and keys
                if event.namespace_path == req.namespace_path {
                    if req.keys.is_empty() || req.keys.contains(&event.key) {
                        yield Ok(event);
                    }
                }
            }
        };
        
        Ok(Response::new(Box::pin(stream) as Self::WatchConfigStream))
    }
}
```

## 4. Schema Validation System

### Requirements
- JSON Schema support
- Type validation
- Range checks
- Custom validators

### Implementation

```rust
// config-store/src/validation/schema.rs

use jsonschema::{JSONSchema, Draft};
use serde_json::Value;

pub struct SchemaValidator {
    schemas: HashMap<String, JSONSchema>,
    custom_validators: HashMap<String, Box<dyn CustomValidator>>,
}

impl SchemaValidator {
    pub fn new() -> Self {
        SchemaValidator {
            schemas: HashMap::new(),
            custom_validators: HashMap::new(),
        }
    }
    
    pub fn register_schema(&mut self, path: &str, schema: Value) -> Result<(), ConfigError> {
        let compiled = JSONSchema::options()
            .with_draft(Draft::Draft7)
            .compile(&schema)
            .map_err(|e| ConfigError::InvalidSchema(e.to_string()))?;
            
        self.schemas.insert(path.to_string(), compiled);
        Ok(())
    }
    
    pub fn validate(&self, path: &str, value: &ConfigValue) -> Result<(), ConfigError> {
        // Check for exact path schema
        if let Some(schema) = self.schemas.get(path) {
            return self.validate_against_schema(value, schema);
        }
        
        // Check for pattern-based schemas
        for (pattern, schema) in &self.schemas {
            if path.starts_with(pattern.trim_end_matches('*')) {
                return self.validate_against_schema(value, schema);
            }
        }
        
        // Check custom validators
        if let Some(validator) = self.custom_validators.get(path) {
            return validator.validate(value);
        }
        
        Ok(())
    }
    
    fn validate_against_schema(&self, value: &ConfigValue, schema: &JSONSchema) -> Result<(), ConfigError> {
        let json_value = serde_json::to_value(value)?;
        
        match schema.validate(&json_value) {
            Ok(()) => Ok(()),
            Err(errors) => {
                let error_messages: Vec<String> = errors
                    .map(|e| format!("{}: {}", e.instance_path, e))
                    .collect();
                    
                Err(ConfigError::ValidationFailed(error_messages))
            }
        }
    }
}

// Custom validators for complex business logic
pub trait CustomValidator: Send + Sync {
    fn validate(&self, value: &ConfigValue) -> Result<(), ConfigError>;
}

pub struct TradingHoursValidator;

impl CustomValidator for TradingHoursValidator {
    fn validate(&self, value: &ConfigValue) -> Result<(), ConfigError> {
        if let ConfigValue::Object(map) = value {
            let open = map.get("market_open")
                .ok_or(ConfigError::ValidationFailed(vec!["market_open required".to_string()]))?;
            let close = map.get("market_close")
                .ok_or(ConfigError::ValidationFailed(vec!["market_close required".to_string()]))?;
                
            // Validate time format
            let time_regex = regex::Regex::new(r"^\d{2}:\d{2}$").unwrap();
            
            if let ConfigValue::String(open_str) = open {
                if !time_regex.is_match(open_str) {
                    return Err(ConfigError::ValidationFailed(vec![
                        "market_open must be in HH:MM format".to_string()
                    ]));
                }
            }
            
            // Additional business logic validation
            // ...
        }
        
        Ok(())
    }
}
```

## 5. ServiceConfig Pattern Implementation

### Requirements
- Type-safe configuration wrapper
- Local caching with TTL
- Automatic refresh
- Validation on load

### Implementation

```rust
// config-store/src/patterns/service_config.rs

use std::marker::PhantomData;
use validator::Validate;

pub struct ServiceConfig<T> 
where
    T: DeserializeOwned + Validate + Clone + Send + Sync + 'static,
{
    store: Arc<dyn ConfigStore>,
    path: String,
    cache: Arc<RwLock<Option<CachedConfig<T>>>>,
    cache_ttl: Duration,
    refresh_interval: Option<Duration>,
    validator: Option<Box<dyn Fn(&T) -> Result<(), ConfigError> + Send + Sync>>,
    _phantom: PhantomData<T>,
}

struct CachedConfig<T> {
    value: T,
    loaded_at: Instant,
}

impl<T> ServiceConfig<T>
where
    T: DeserializeOwned + Validate + Clone + Send + Sync + 'static,
{
    pub fn new(store: Arc<dyn ConfigStore>, path: impl Into<String>) -> Self {
        ServiceConfig {
            store,
            path: path.into(),
            cache: Arc::new(RwLock::new(None)),
            cache_ttl: Duration::from_secs(60),
            refresh_interval: None,
            validator: None,
            _phantom: PhantomData,
        }
    }
    
    pub fn with_cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }
    
    pub fn with_refresh_interval(mut self, interval: Duration) -> Self {
        self.refresh_interval = Some(interval);
        self
    }
    
    pub fn with_validator<F>(mut self, validator: F) -> Self
    where
        F: Fn(&T) -> Result<(), ConfigError> + Send + Sync + 'static,
    {
        self.validator = Some(Box::new(validator));
        self
    }
    
    pub async fn load(&self) -> Result<T, ConfigError> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = &*cache {
                if cached.loaded_at.elapsed() < self.cache_ttl {
                    return Ok(cached.value.clone());
                }
            }
        }
        
        // Load from store
        let value = self.store.get(&self.path).await?;
        
        // Deserialize
        let typed_value: T = serde_json::from_value(serde_json::to_value(value)?)?;
        
        // Validate using Validate trait
        typed_value.validate()
            .map_err(|e| ConfigError::ValidationFailed(
                e.field_errors().into_iter()
                    .flat_map(|(field, errors)| {
                        errors.iter().map(move |e| format!("{}: {}", field, e.message.as_ref().unwrap_or(&e.code)))
                    })
                    .collect()
            ))?;
        
        // Custom validation
        if let Some(validator) = &self.validator {
            validator(&typed_value)?;
        }
        
        // Update cache
        {
            let mut cache = self.cache.write().await;
            *cache = Some(CachedConfig {
                value: typed_value.clone(),
                loaded_at: Instant::now(),
            });
        }
        
        Ok(typed_value)
    }
    
    pub async fn reload(&self) -> Result<T, ConfigError> {
        // Force cache invalidation
        {
            let mut cache = self.cache.write().await;
            *cache = None;
        }
        
        self.load().await
    }
    
    pub async fn watch<F>(&self, mut callback: F) -> Result<(), ConfigError>
    where
        F: FnMut(&T) + Send + 'static,
    {
        let interval = self.refresh_interval
            .unwrap_or(Duration::from_secs(30));
            
        let mut current = self.load().await?;
        callback(&current);
        
        let mut ticker = tokio::time::interval(interval);
        
        loop {
            ticker.tick().await;
            
            match self.load().await {
                Ok(new_value) => {
                    if !self.values_equal(&current, &new_value) {
                        current = new_value;
                        callback(&current);
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to reload config: {}", e);
                }
            }
        }
    }
    
    fn values_equal(&self, a: &T, b: &T) -> bool {
        // Use serde for comparison
        serde_json::to_value(a).ok() == serde_json::to_value(b).ok()
    }
}
```

## 6. Comprehensive Test Suite

### Unit Tests (100% Coverage Required)

```rust
// config-store/tests/unit/

#[cfg(test)]
mod tests {
    use super::*;
    
    // Test every public method
    // Test error conditions
    // Test edge cases
    // Test concurrent access
    // Test validation logic
}
```

### Integration Tests

```rust
// config-store/tests/integration/

#[cfg(test)]
mod redis_integration_tests {
    use testcontainers::*;
    
    #[tokio::test]
    async fn test_redis_full_lifecycle() {
        // Setup Redis container
        // Test all CRUD operations
        // Test transactions
        // Test versioning
        // Test performance
    }
}
```

### Performance Tests

```rust
// config-store/tests/performance/

#[cfg(test)]
mod performance_tests {
    use criterion::{black_box, criterion_group, criterion_main, Criterion};
    
    fn benchmark_reads(c: &mut Criterion) {
        c.bench_function("redis_read_single", |b| {
            b.iter(|| {
                // Measure single read latency
            });
        });
        
        c.bench_function("redis_read_concurrent", |b| {
            b.iter(|| {
                // Measure concurrent read throughput
            });
        });
    }
    
    criterion_group!(benches, benchmark_reads);
    criterion_main!(benches);
}
```

## 7. Migration & Integration

### ENV Migration Tool

```rust
// config-store/src/migration/env_migrator.rs

pub struct EnvMigrator {
    mapping_rules: HashMap<String, MappingRule>,
    validators: HashMap<String, Box<dyn Validator>>,
}

struct MappingRule {
    target_path: String,
    transform: Option<Box<dyn Fn(String) -> ConfigValue>>,
}

impl EnvMigrator {
    pub async fn migrate(
        &self,
        env_file: &Path,
        store: &dyn ConfigStore,
        dry_run: bool,
    ) -> Result<MigrationReport, ConfigError> {
        // Parse .env file
        // Apply mapping rules
        // Validate all values
        // Execute migration (or dry run)
        // Return detailed report
    }
}
```

## 8. Docker Environment

```dockerfile
# Dockerfile for config-store
FROM rust:1.82-slim as builder

WORKDIR /build
COPY . .
RUN cargo build --release --bin config-store-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/config-store-server /usr/local/bin/
COPY --from=builder /build/proto /proto

EXPOSE 50051
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD grpc_health_probe -addr=:50051 || exit 1

CMD ["config-store-server"]
```

```yaml
# docker-compose.test.yml
version: '3.8'

services:
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 5s
      timeout: 3s
      retries: 5
      
  config-store:
    build: .
    depends_on:
      redis:
        condition: service_healthy
    environment:
      REDIS_URL: redis://redis:6379
      LOG_LEVEL: debug
    ports:
      - "50051:50051"
    volumes:
      - ./configs:/configs
```

## 9. Security Features Integration

### Preserve and Enhance

```rust
// config-store/src/security/

pub struct SecurityLayer {
    rate_limiter: RateLimiter,
    sanitizer: InputSanitizer,
    blocklist: Blocklist,
    encryption: EncryptionHandler,
}

impl SecurityLayer {
    pub async fn validate_request(&self, request: &Request) -> Result<(), SecurityError> {
        // Rate limiting
        self.rate_limiter.check(request.client_ip)?;
        
        // Input sanitization
        let sanitized = self.sanitizer.sanitize(request.body)?;
        
        // Blocklist check
        self.blocklist.check(&sanitized)?;
        
        Ok(())
    }
    
    pub async fn encrypt_sensitive(&self, value: &mut ConfigValue) -> Result<(), SecurityError> {
        // Encrypt fields marked as sensitive
        // Store encryption metadata
        Ok(())
    }
}
```

## 10. Quality Metrics & Monitoring

### Required Metrics
- Read latency P50, P95, P99
- Write latency P50, P95, P99
- Cache hit rate
- Error rate by type
- Active connections
- Memory usage
- Version history size

### Prometheus Integration

```rust
use prometheus::{register_histogram_vec, register_counter_vec, register_gauge};

lazy_static! {
    static ref READ_LATENCY: HistogramVec = register_histogram_vec!(
        "config_store_read_latency_seconds",
        "Read operation latency",
        &["backend"]
    ).unwrap();
    
    static ref CACHE_HITS: CounterVec = register_counter_vec!(
        "config_store_cache_hits_total",
        "Cache hit count",
        &["backend"]
    ).unwrap();
}
```

## Implementation Timeline

### Week 1: Core Components
- **Day 1-2**: Fix gRPC server, complete proto integration
- **Day 3-4**: Implement RedisConfigStore with full feature set
- **Day 5**: Implement FileConfigStore with hot-reload

### Week 2: Advanced Features
- **Day 6-7**: Schema validation system
- **Day 8**: ServiceConfig pattern
- **Day 9**: Real-time streaming
- **Day 10**: Version management

### Week 3: Testing & Integration
- **Day 11-12**: Comprehensive test suite
- **Day 13**: Performance testing & optimization
- **Day 14**: Migration tools
- **Day 15**: Documentation & deployment

## Success Criteria

1. **Functional Completeness**
   - ✅ All ConfigStore trait methods implemented
   - ✅ RedisConfigStore with all features
   - ✅ FileConfigStore with hot-reload
   - ✅ Schema validation working
   - ✅ Real-time streaming operational
   - ✅ Version tracking (last 10 versions)

2. **Performance Requirements Met**
   - ✅ < 10ms read latency (P95)
   - ✅ < 50ms write latency (P95)
   - ✅ 10,000 reads/second capability
   - ✅ > 90% cache hit rate

3. **Quality Standards**
   - ✅ 100% test coverage for business logic
   - ✅ All integration tests passing
   - ✅ Performance benchmarks met
   - ✅ Security features preserved and enhanced
   - ✅ Zero memory leaks
   - ✅ Full documentation

## Conclusion

This plan provides a complete roadmap to restore config-store to 100% functionality with production-quality implementation. Every component will be built with:
- Test-first development
- Full specification compliance
- Performance optimization
- Security enhancement
- Comprehensive documentation

No shortcuts will be taken. The implementation will follow industry best practices and meet all requirements from the original specification plus the enhanced security features added post-specification.