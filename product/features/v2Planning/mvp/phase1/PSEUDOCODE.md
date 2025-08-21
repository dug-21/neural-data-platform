# Phase 1: Configuration Foundation - Pseudocode with TDD Anchors

## 1. Test-Driven Development Flow

### 1.1 TDD Cycle for Each Component
```
FOR each component:
    1. WRITE failing test (RED)
    2. IMPLEMENT minimal code to pass (GREEN)
    3. REFACTOR for quality (REFACTOR)
    4. DOCUMENT patterns used
```

## 2. ConfigStore Trait Implementation

### 2.1 Test First: ConfigStore Interface
```pseudocode
TEST "ConfigStore trait contract":
    GIVEN a ConfigStore implementation
    WHEN calling any trait method
    THEN it must return Result<T, ConfigError>
    AND be Send + Sync for async usage
    
TEST "ConfigStore get operation":
    GIVEN a store with "/system/trading_hours" = {"open": "09:30"}
    WHEN get("/system/trading_hours")
    THEN return ConfigValue with correct data
    
TEST "ConfigStore get missing key":
    GIVEN an empty store
    WHEN get("/nonexistent/path")
    THEN return ConfigError::NotFound
```

### 2.2 Implementation: ConfigStore Trait
```pseudocode
TRAIT ConfigStore:
    // Core CRUD Operations
    ASYNC FUNCTION get(path: String) -> Result<ConfigValue>:
        VALIDATE path format
        RETURN implementation-specific retrieval
        
    ASYNC FUNCTION set(path: String, value: ConfigValue) -> Result<()>:
        VALIDATE path format
        VALIDATE value against schema if exists
        INCREMENT version counter
        STORE with metadata
        TRIGGER change notifications
        RETURN success or error
        
    ASYNC FUNCTION delete(path: String) -> Result<()>:
        CHECK if path exists
        ARCHIVE to history
        REMOVE from active store
        RETURN success or error
        
    // Bulk Operations
    ASYNC FUNCTION get_tree(prefix: String) -> Result<ConfigTree>:
        FIND all keys with prefix
        BUILD hierarchical tree structure
        RESOLVE inheritance relationships
        RETURN merged configuration tree
```

## 3. RedisConfigStore Implementation

### 3.1 Test First: Redis Backend
```pseudocode
TEST "RedisConfigStore connection":
    GIVEN Redis connection parameters
    WHEN creating RedisConfigStore
    THEN successfully connect or return error
    
TEST "RedisConfigStore atomic operations":
    GIVEN multiple concurrent updates
    WHEN executing in transaction
    THEN all succeed or all rollback
    
TEST "RedisConfigStore performance":
    GIVEN 1000 configuration keys
    WHEN reading single key
    THEN complete in < 10ms
```

### 3.2 Implementation: RedisConfigStore
```pseudocode
CLASS RedisConfigStore:
    PRIVATE:
        redis_client: RedisConnection
        key_prefix: String = "config:"
        
    FUNCTION new(connection_url: String) -> Result<Self>:
        TRY:
            client = Redis.connect(connection_url)
            PING to verify connection
            RETURN RedisConfigStore { redis_client: client }
        CATCH error:
            RETURN ConfigError::ConnectionFailed(error)
            
    ASYNC FUNCTION get(path: String) -> Result<ConfigValue>:
        redis_key = format_redis_key(path)
        
        TRY:
            // Check local cache first (with TTL)
            IF cached_value = cache.get(redis_key) AND not expired:
                RETURN cached_value
                
            // Fetch from Redis
            raw_value = redis_client.get(redis_key).await?
            
            IF raw_value is None:
                RETURN ConfigError::NotFound(path)
                
            // Deserialize and validate
            config_value = deserialize_json(raw_value)?
            
            // Handle inheritance
            IF config_value.has_inheritance():
                parent_configs = fetch_parent_configs(config_value.inheritance)
                config_value = merge_configs(parent_configs, config_value)
                
            // Update cache
            cache.set(redis_key, config_value, TTL=60s)
            
            RETURN Ok(config_value)
            
        CATCH error:
            LOG error with context
            RETURN ConfigError::RetrievalFailed(error)
            
    ASYNC FUNCTION set(path: String, value: ConfigValue) -> Result<()>:
        redis_key = format_redis_key(path)
        
        START transaction:
            // Get current version
            current = redis_client.get(redis_key)
            new_version = current.version + 1 if current else 1
            
            // Prepare versioned value
            versioned_value = ConfigNode {
                path: path,
                value: value,
                version: new_version,
                metadata: {
                    updated_at: now(),
                    updated_by: current_user()
                }
            }
            
            // Store current version in history
            IF current exists:
                history_key = format_history_key(path, current.version)
                redis_client.set(history_key, current, EXPIRE=30days)
                
            // Set new value
            redis_client.set(redis_key, serialize(versioned_value))
            
            // Invalidate cache
            cache.invalidate(redis_key)
            broadcast_invalidation(redis_key)
            
        COMMIT transaction or ROLLBACK on error
        
        RETURN success or error
```

## 4. InMemoryConfigStore for Testing

### 4.1 Test First: InMemory Implementation
```pseudocode
TEST "InMemoryConfigStore isolation":
    GIVEN two InMemoryConfigStore instances
    WHEN setting value in one
    THEN other instance should not see the change
    
TEST "InMemoryConfigStore deterministic":
    GIVEN same sequence of operations
    WHEN replaying on new instance
    THEN final state must be identical
```

### 4.2 Implementation: InMemoryConfigStore
```pseudocode
CLASS InMemoryConfigStore:
    PRIVATE:
        data: HashMap<String, ConfigNode>
        history: HashMap<(String, Version), ConfigNode>
        lock: RwLock
        
    FUNCTION new() -> Self:
        RETURN InMemoryConfigStore {
            data: empty HashMap,
            history: empty HashMap,
            lock: new RwLock
        }
        
    ASYNC FUNCTION get(path: String) -> Result<ConfigValue>:
        ACQUIRE read lock
        
        IF value = data.get(path):
            RETURN Ok(value.clone())
        ELSE:
            RETURN ConfigError::NotFound(path)
            
    ASYNC FUNCTION set(path: String, value: ConfigValue) -> Result<()>:
        ACQUIRE write lock
        
        // Same versioning logic as Redis
        current = data.get(path)
        new_version = increment_version(current)
        
        IF current exists:
            history.insert((path, current.version), current)
            
        data.insert(path, ConfigNode::new(value, new_version))
        
        RETURN Ok(())
        
    // Helper for testing
    FUNCTION snapshot() -> ConfigSnapshot:
        RETURN deep_clone(data)
        
    FUNCTION restore(snapshot: ConfigSnapshot):
        data = snapshot
```

## 5. ServiceConfig Integration Pattern

### 5.1 Test First: Service Integration
```pseudocode
TEST "ServiceConfig caching":
    GIVEN ServiceConfig with 60s TTL
    WHEN calling load() twice within 60s
    THEN second call should not hit store
    
TEST "ServiceConfig validation":
    GIVEN ServiceConfig with validator
    WHEN loading invalid configuration
    THEN return validation error
    
TEST "ServiceConfig refresh":
    GIVEN cached configuration
    WHEN calling refresh()
    THEN fetch fresh from store regardless of cache
```

### 5.2 Implementation: ServiceConfig Pattern
```pseudocode
CLASS ServiceConfig<T>:
    PRIVATE:
        store: Arc<ConfigStore>
        path: String
        cache: RwLock<Option<(T, Instant)>>
        cache_ttl: Duration
        validator: Validator<T>
        
    FUNCTION new(store, path, cache_ttl) -> Self:
        RETURN ServiceConfig {
            store: Arc::new(store),
            path: path,
            cache: RwLock::new(None),
            cache_ttl: cache_ttl,
            validator: default_validator()
        }
        
    ASYNC FUNCTION load() -> Result<T>:
        // Check cache first
        IF cached = read_cache() AND not expired:
            RETURN Ok(cached.value)
            
        // Load from store
        raw_value = store.get(path).await?
        
        // Deserialize to type T
        typed_value: T = deserialize(raw_value)?
        
        // Validate
        validation_result = validator.validate(&typed_value)
        IF validation_result.is_error():
            RETURN ConfigError::ValidationFailed(validation_result.errors)
            
        // Update cache
        write_cache(typed_value, now())
        
        RETURN Ok(typed_value)
        
    ASYNC FUNCTION refresh() -> Result<T>:
        // Force cache invalidation
        clear_cache()
        RETURN load().await
        
    ASYNC FUNCTION watch(callback: Function):
        LOOP:
            old_value = load().await?
            SLEEP refresh_interval
            new_value = load().await?
            
            IF old_value != new_value:
                callback(new_value)
```

## 6. Configuration Hierarchy and Inheritance

### 6.1 Test First: Inheritance Logic
```pseudocode
TEST "Configuration inheritance":
    GIVEN parent config at /system/global with {timeout: 30}
    AND child config at /system/global/trading with {retries: 3}
    WHEN loading /system/global/trading with inheritance
    THEN result should have {timeout: 30, retries: 3}
    
TEST "Override precedence":
    GIVEN parent with {timeout: 30}
    AND child with {timeout: 60}
    WHEN merging
    THEN child value takes precedence {timeout: 60}
```

### 6.2 Implementation: Inheritance Resolution
```pseudocode
FUNCTION resolve_inheritance(store: ConfigStore, path: String) -> ConfigValue:
    config = store.get(path)?
    
    IF not config.has_inheritance():
        RETURN config
        
    // Collect all parent configs
    parent_configs = []
    FOR parent_path in config.inheritance:
        parent = resolve_inheritance(store, parent_path)  // Recursive
        parent_configs.append(parent)
        
    // Merge in order (parents first, child last)
    merged = ConfigValue::new()
    FOR parent in parent_configs:
        merged = merge_configs(merged, parent)
    merged = merge_configs(merged, config)
    
    RETURN merged
    
FUNCTION merge_configs(base: ConfigValue, override: ConfigValue) -> ConfigValue:
    result = deep_clone(base)
    
    FOR key, value in override:
        IF value is Object AND result[key] is Object:
            // Recursive merge for nested objects
            result[key] = merge_configs(result[key], value)
        ELSE:
            // Override takes precedence
            result[key] = value
            
    RETURN result
```

## 7. Migration from .env Files

### 7.1 Test First: ENV Migration
```pseudocode
TEST "ENV variable mapping":
    GIVEN .env with DATABASE_URL=postgres://localhost
    WHEN migrating with mapping rules
    THEN creates /infrastructure/storage/database/url = "postgres://localhost"
    
TEST "ENV validation during migration":
    GIVEN .env with invalid values
    WHEN migrating
    THEN report validation errors without partial migration
```

### 7.2 Implementation: ENV Migration
```pseudocode
CLASS EnvMigrator:
    PRIVATE:
        mapping_rules: HashMap<String, String>
        validators: HashMap<String, Validator>
        
    FUNCTION migrate(env_file: Path, store: ConfigStore) -> Result<MigrationReport>:
        // Load and parse .env file
        env_vars = parse_env_file(env_file)?
        
        // Build migration plan
        migration_plan = []
        errors = []
        
        FOR key, value in env_vars:
            IF target_path = mapping_rules.get(key):
                // Validate before adding to plan
                IF validator = validators.get(target_path):
                    IF validation_error = validator.validate(value):
                        errors.append(validation_error)
                        CONTINUE
                        
                migration_plan.append({
                    source: key,
                    target: target_path,
                    value: transform_value(value)
                })
            ELSE:
                warnings.append(f"No mapping for {key}")
                
        // Execute migration if no errors
        IF errors.is_empty():
            START transaction:
                FOR item in migration_plan:
                    store.set(item.target, item.value).await?
            COMMIT
            
            RETURN Ok(MigrationReport {
                migrated: migration_plan.len(),
                warnings: warnings
            })
        ELSE:
            RETURN Err(MigrationError { errors: errors })
```

## 8. Docker Test Environment

### 8.1 Test Environment Setup
```pseudocode
FUNCTION setup_test_environment():
    // Docker Compose for test dependencies
    docker_compose = """
    services:
      redis-test:
        image: redis:7-alpine
        ports: ["6379:6379"]
        
      config-store-test:
        build: .
        environment:
          REDIS_URL: redis://redis-test:6379
          TEST_MODE: true
    """
    
    // Start services
    docker_compose_up(docker_compose)
    
    // Wait for health checks
    WAIT_UNTIL redis_test.is_healthy() TIMEOUT 30s
    
    // Run test suite
    RUN cargo test --all-features
    
    // Cleanup
    docker_compose_down()
```

## 9. Performance Testing

### 9.1 Load Test Scenarios
```pseudocode
TEST "Concurrent read performance":
    GIVEN ConfigStore with 1000 keys loaded
    WHEN spawning 100 concurrent readers
    THEN 95th percentile latency < 10ms
    AND no errors
    
TEST "Write throughput":
    GIVEN empty ConfigStore
    WHEN writing 1000 keys sequentially
    THEN complete in < 10 seconds
    AND all keys retrievable
    
TEST "Cache effectiveness":
    GIVEN ServiceConfig with 60s TTL
    WHEN reading same key 1000 times
    THEN cache hit rate > 99%
    AND average latency < 1ms
```

## 10. Integration Test Suite

### 10.1 End-to-End Test Flow
```pseudocode
TEST "Complete configuration lifecycle":
    // Setup
    store = create_test_store()
    
    // Test creation
    store.set("/test/config", {value: 1})
    ASSERT store.get("/test/config").value == 1
    
    // Test update with versioning
    store.set("/test/config", {value: 2})
    ASSERT store.get("/test/config").value == 2
    ASSERT store.get_version("/test/config", 1).value == 1
    
    // Test deletion
    store.delete("/test/config")
    ASSERT store.get("/test/config") returns NotFound
    
    // Test tree operations
    store.set("/test/a", {x: 1})
    store.set("/test/b", {y: 2})
    tree = store.get_tree("/test")
    ASSERT tree contains both configs
    
    // Test transactions
    TRY transaction:
        store.set("/test/tx1", {})
        store.set("/test/tx2", {})
        THROW error  // Simulate failure
    CATCH:
        ASSERT neither tx1 nor tx2 exist
```

---

*Pseudocode Version*: 1.0
*Created*: 2025-01-20
*TDD Approach*: Red-Green-Refactor for all components
*Next Step*: Begin implementation with failing tests