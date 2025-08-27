# Neural Trader V2 - Binary Separation Testing Implementation Examples

## Overview

Real-world implementation examples demonstrating how to apply the comprehensive testing strategy for the **binary separation architecture** with Redis Streams integration.

## Binary Testing Examples Catalog

### 1. Config Store Binary Testing (Rust + gRPC)
### 2. Data Ingestion Binary Testing (Python + Redis Streams)
### 3. ruv-FANN Binary Testing (Rust + Neural Networks)
### 4. DAA Coordinator Binary Testing (Rust + Distributed Agents)
### 5. Redis Streams Cross-Binary Communication Testing
### 6. Binary Integration Testing Scenarios

## 1. Config Store Binary Testing Example (Rust)

### gRPC Service Implementation
```rust
// config-store/src/service.rs
use tonic::{Request, Response, Status};
use sqlx::PgPool;
use std::sync::Arc;

use crate::proto::config_store::{
    config_store_server::ConfigStore,
    GetConfigRequest, GetConfigResponse, SetConfigRequest, SetConfigResponse,
    ConfigEntry,
};

#[derive(Clone)]
pub struct ConfigStoreService {
    db_pool: Arc<PgPool>,
    redis_client: Arc<redis::Client>,
}

impl ConfigStoreService {
    pub fn new(db_pool: PgPool, redis_client: redis::Client) -> Self {
        ConfigStoreService {
            db_pool: Arc::new(db_pool),
            redis_client: Arc::new(redis_client),
        }
    }
}

#[tonic::async_trait]
impl ConfigStore for ConfigStoreService {
    async fn get_config(
        &self,
        request: Request<GetConfigRequest>,
    ) -> Result<Response<GetConfigResponse>, Status> {
        let req = request.into_inner();
        
        // First check Redis cache
        let cache_key = format!("config:{}:{}", req.namespace, req.key);
        let mut redis_conn = self.redis_client.get_async_connection().await
            .map_err(|e| Status::internal(format!("Redis error: {}", e)))?;
            
        if let Ok(cached_value) = redis::cmd("GET")
            .arg(&cache_key)
            .query_async::<_, Option<String>>(&mut redis_conn)
            .await
        {
            if let Some(cached) = cached_value {
                if let Ok(config) = serde_json::from_str::<ConfigEntry>(&cached) {
                    return Ok(Response::new(GetConfigResponse {
                        config: Some(config),
                    }));
                }
            }
        }
        
        // Fallback to database
        let config = sqlx::query_as!(
            ConfigEntry,
            "SELECT key, value, namespace, version, created_at, updated_at 
             FROM configurations 
             WHERE key = $1 AND namespace = $2",
            req.key,
            req.namespace
        )
        .fetch_optional(&*self.db_pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;
        
        match config {
            Some(config) => {
                // Cache for future requests
                let config_json = serde_json::to_string(&config)
                    .map_err(|e| Status::internal(format!("Serialization error: {}", e)))?;
                    
                let _: () = redis::cmd("SETEX")
                    .arg(&cache_key)
                    .arg(300) // 5 minutes TTL
                    .arg(&config_json)
                    .query_async(&mut redis_conn)
                    .await
                    .unwrap_or_default(); // Don't fail if cache fails
                    
                Ok(Response::new(GetConfigResponse {
                    config: Some(config),
                }))
            }
            None => Err(Status::not_found("Configuration not found")),
        }
    }
    
    async fn set_config(
        &self,
        request: Request<SetConfigRequest>,
    ) -> Result<Response<SetConfigResponse>, Status> {
        let req = request.into_inner();
        
        // Insert/update in database
        let config = sqlx::query_as!(
            ConfigEntry,
            "INSERT INTO configurations (key, value, namespace, version, created_at, updated_at)
             VALUES ($1, $2, $3, 1, NOW(), NOW())
             ON CONFLICT (key, namespace) DO UPDATE SET
                 value = EXCLUDED.value,
                 version = configurations.version + 1,
                 updated_at = NOW()
             RETURNING key, value, namespace, version, created_at, updated_at",
            req.key,
            req.value,
            req.namespace
        )
        .fetch_one(&*self.db_pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;
        
        // Update cache
        let cache_key = format!("config:{}:{}", req.namespace, req.key);
        let config_json = serde_json::to_string(&config)
            .map_err(|e| Status::internal(format!("Serialization error: {}", e)))?;
            
        if let Ok(mut redis_conn) = self.redis_client.get_async_connection().await {
            let _: () = redis::cmd("SETEX")
                .arg(&cache_key)
                .arg(300)
                .arg(&config_json)
                .query_async(&mut redis_conn)
                .await
                .unwrap_or_default();
        }
        
        // Publish configuration change to Redis Streams
        self.publish_config_change(&config).await?;
        
        Ok(Response::new(SetConfigResponse {
            config: Some(config),
        }))
    }
}

impl ConfigStoreService {
    async fn publish_config_change(&self, config: &ConfigEntry) -> Result<(), Status> {
        if let Ok(mut redis_conn) = self.redis_client.get_async_connection().await {
            let change_event = serde_json::json!({
                "type": "config_updated",
                "key": config.key,
                "namespace": config.namespace,
                "version": config.version,
                "timestamp": chrono::Utc::now().timestamp()
            });
            
            let _: () = redis::cmd("XADD")
                .arg("config_updates_stream")
                .arg("*")
                .arg("event")
                .arg(change_event.to_string())
                .query_async(&mut redis_conn)
                .await
                .map_err(|e| Status::internal(format!("Stream publish error: {}", e)))?;
        }
        
        Ok(())
    }
}
```
  constructor(
    private repository: ConfigRepository,
    private cache: CacheService,
    private validator: ConfigValidator
  ) {}

  async getConfig(key: string, namespace: string = 'default'): Promise<ConfigEntry | null> {
    // Check cache first
    const cacheKey = `config:${namespace}:${key}`;
    const cached = await this.cache.get(cacheKey);
    
    if (cached) {
      return JSON.parse(cached);
    }

    // Fetch from database
    const config = await this.repository.findByKey(key, namespace);
    
    if (config) {
      // Cache for 5 minutes
      await this.cache.set(cacheKey, JSON.stringify(config), 300);
    }

    return config;
  }

  async setConfig(
    key: string, 
    value: any, 
    namespace: string = 'default',
    metadata: ConfigMetadata = {}
  ): Promise<ConfigEntry> {
    // Validate configuration
    const validationResult = await this.validator.validate(key, value, namespace);
    if (!validationResult.isValid) {
      throw new ValidationError(`Invalid configuration: ${validationResult.errors.join(', ')}`);
    }

    // Create or update config
    const configEntry: ConfigEntry = {
      key,
      value,
      namespace,
      version: await this.getNextVersion(key, namespace),
      createdAt: new Date(),
      updatedAt: new Date(),
      metadata
    };

    const savedConfig = await this.repository.save(configEntry);
    
    // Update cache
    const cacheKey = `config:${namespace}:${key}`;
    await this.cache.set(cacheKey, JSON.stringify(savedConfig), 300);
    
    // Publish configuration change event
    await this.publishConfigChangeEvent(savedConfig);

    return savedConfig;
  }

  private async getNextVersion(key: string, namespace: string): Promise<number> {
    const latestConfig = await this.repository.findByKey(key, namespace);
    return latestConfig ? latestConfig.version + 1 : 1;
  }

  private async publishConfigChangeEvent(config: ConfigEntry): Promise<void> {
    // Implementation for event publishing
  }
}
```

### Complete Rust Unit Test Suite
```rust
// config-store/src/service.rs
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;
    use tokio_test;
    use sqlx::PgPool;
    use testcontainers::{
        clients::Cli,
        images::{postgres::Postgres, redis::Redis},
        Container,
    };
    
    struct TestContext {
        db_pool: PgPool,
        redis_client: redis::Client,
        _postgres_container: Container<'static, Postgres>,
        _redis_container: Container<'static, Redis>,
    }
    
    impl TestContext {
        async fn new() -> Self {
            let docker = Cli::default();
            
            // Start test containers
            let postgres_container = docker.run(Postgres::default());
            let redis_container = docker.run(Redis::default());
            
            let postgres_port = postgres_container.get_host_port_ipv4(5432);
            let redis_port = redis_container.get_host_port_ipv4(6379);
            
            // Setup database connection
            let database_url = format!(
                "postgres://postgres:postgres@localhost:{}/postgres",
                postgres_port
            );
            let db_pool = PgPool::connect(&database_url).await.unwrap();
            
            // Run migrations
            sqlx::migrate!("../migrations").run(&db_pool).await.unwrap();
            
            // Setup Redis connection
            let redis_url = format!("redis://localhost:{}", redis_port);
            let redis_client = redis::Client::open(redis_url).unwrap();
            
            TestContext {
                db_pool,
                redis_client,
                _postgres_container: postgres_container,
                _redis_container: redis_container,
            }
        }
    }
    
    #[tokio::test]
    async fn test_get_config_from_cache() {
        let ctx = TestContext::new().await;
        let service = ConfigStoreService::new(ctx.db_pool.clone(), ctx.redis_client.clone());
        
        // Arrange - Seed Redis cache
        let mut redis_conn = ctx.redis_client.get_async_connection().await.unwrap();
        let test_config = ConfigEntry {
            key: "test_key".to_string(),
            value: "test_value".to_string(),
            namespace: "test".to_string(),
            version: 1,
            created_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
            updated_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
        };
        
        let cache_key = "config:test:test_key";
        let config_json = serde_json::to_string(&test_config).unwrap();
        let _: () = redis::cmd("SET")
            .arg(cache_key)
            .arg(&config_json)
            .query_async(&mut redis_conn)
            .await
            .unwrap();
        
        // Act
        let request = tonic::Request::new(GetConfigRequest {
            key: "test_key".to_string(),
            namespace: "test".to_string(),
        });
        
        let response = service.get_config(request).await;
        
        // Assert
        assert!(response.is_ok());
        let response = response.unwrap().into_inner();
        assert!(response.config.is_some());
        
        let config = response.config.unwrap();
        assert_eq!(config.key, "test_key");
        assert_eq!(config.value, "test_value");
        assert_eq!(config.namespace, "test");
        assert_eq!(config.version, 1);
    }
    
    #[tokio::test]
    async fn test_get_config_from_database_when_cache_miss() {
        let ctx = TestContext::new().await;
        let service = ConfigStoreService::new(ctx.db_pool.clone(), ctx.redis_client.clone());
        
        // Arrange - Seed database directly
        sqlx::query!(
            "INSERT INTO configurations (key, value, namespace, version, created_at, updated_at)
             VALUES ($1, $2, $3, $4, NOW(), NOW())",
            "db_key",
            "db_value",
            "test",
            1
        )
        .execute(&ctx.db_pool)
        .await
        .unwrap();
        
        // Act
        let request = tonic::Request::new(GetConfigRequest {
            key: "db_key".to_string(),
            namespace: "test".to_string(),
        });
        
        let response = service.get_config(request).await;
        
        // Assert
        assert!(response.is_ok());
        let response = response.unwrap().into_inner();
        assert!(response.config.is_some());
        
        let config = response.config.unwrap();
        assert_eq!(config.key, "db_key");
        assert_eq!(config.value, "db_value");
        
        // Verify cache was populated
        let mut redis_conn = ctx.redis_client.get_async_connection().await.unwrap();
        let cached_value: Option<String> = redis::cmd("GET")
            .arg("config:test:db_key")
            .query_async(&mut redis_conn)
            .await
            .unwrap();
            
        assert!(cached_value.is_some());
        let cached_config: ConfigEntry = serde_json::from_str(&cached_value.unwrap()).unwrap();
        assert_eq!(cached_config.key, "db_key");
    }
    
    #[tokio::test]
    async fn test_set_config_publishes_to_stream() {
        let ctx = TestContext::new().await;
        let service = ConfigStoreService::new(ctx.db_pool.clone(), ctx.redis_client.clone());
        
        // Arrange - Create consumer group for monitoring
        let mut redis_conn = ctx.redis_client.get_async_connection().await.unwrap();
        let _: () = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg("config_updates_stream")
            .arg("test_group")
            .arg("0")
            .arg("MKSTREAM")
            .query_async(&mut redis_conn)
            .await
            .unwrap_or_default();
        
        // Act
        let request = tonic::Request::new(SetConfigRequest {
            key: "stream_test_key".to_string(),
            value: "stream_test_value".to_string(),
            namespace: "test".to_string(),
        });
        
        let response = service.set_config(request).await;
        
        // Assert - Config was saved
        assert!(response.is_ok());
        let response = response.unwrap().into_inner();
        assert!(response.config.is_some());
        
        let config = response.config.unwrap();
        assert_eq!(config.key, "stream_test_key");
        assert_eq!(config.value, "stream_test_value");
        
        // Assert - Stream message was published
        let stream_messages: redis::Value = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg("test_group")
            .arg("test_consumer")
            .arg("COUNT")
            .arg(1)
            .arg("STREAMS")
            .arg("config_updates_stream")
            .arg(">")
            .query_async(&mut redis_conn)
            .await
            .unwrap();
            
        // Verify stream message format (simplified assertion)
        match stream_messages {
            redis::Value::Bulk(ref streams) if !streams.is_empty() => {
                // Message was published successfully
                assert!(true);
            }
            _ => panic!("No stream message found"),
        }
    }
    
    #[tokio::test]
    async fn test_config_not_found() {
        let ctx = TestContext::new().await;
        let service = ConfigStoreService::new(ctx.db_pool.clone(), ctx.redis_client.clone());
        
        // Act
        let request = tonic::Request::new(GetConfigRequest {
            key: "nonexistent_key".to_string(),
            namespace: "test".to_string(),
        });
        
        let response = service.get_config(request).await;
        
        // Assert
        assert!(response.is_err());
        let error = response.unwrap_err();
        assert_eq!(error.code(), tonic::Code::NotFound);
        assert!(error.message().contains("Configuration not found"));
    }
    
    #[tokio::test]
    async fn test_config_versioning() {
        let ctx = TestContext::new().await;
        let service = ConfigStoreService::new(ctx.db_pool.clone(), ctx.redis_client.clone());
        
        // Create initial config
        let request1 = tonic::Request::new(SetConfigRequest {
            key: "version_test".to_string(),
            value: "initial_value".to_string(),
            namespace: "test".to_string(),
        });
        
        let response1 = service.set_config(request1).await.unwrap().into_inner();
        assert_eq!(response1.config.unwrap().version, 1);
        
        // Update the same config
        let request2 = tonic::Request::new(SetConfigRequest {
            key: "version_test".to_string(),
            value: "updated_value".to_string(),
            namespace: "test".to_string(),
        });
        
        let response2 = service.set_config(request2).await.unwrap().into_inner();
        let updated_config = response2.config.unwrap();
        assert_eq!(updated_config.version, 2);
        assert_eq!(updated_config.value, "updated_value");
    }
    
    #[tokio::test]
    async fn test_database_error_handling() {
        let ctx = TestContext::new().await;
        
        // Close the database pool to simulate connection failure
        ctx.db_pool.close().await;
        
        let service = ConfigStoreService::new(ctx.db_pool.clone(), ctx.redis_client.clone());
        
        let request = tonic::Request::new(GetConfigRequest {
            key: "test_key".to_string(),
            namespace: "test".to_string(),
        });
        
        let response = service.get_config(request).await;
        
        assert!(response.is_err());
        let error = response.unwrap_err();
        assert_eq!(error.code(), tonic::Code::Internal);
        assert!(error.message().contains("Database error"));
    }
    
    #[tokio::test]
    async fn test_redis_fallback_behavior() {
        let ctx = TestContext::new().await;
        
        // Use invalid Redis client to simulate Redis failure
        let invalid_redis_client = redis::Client::open("redis://localhost:9999").unwrap();
        let service = ConfigStoreService::new(ctx.db_pool.clone(), invalid_redis_client);
        
        // Seed database
        sqlx::query!(
            "INSERT INTO configurations (key, value, namespace, version, created_at, updated_at)
             VALUES ($1, $2, $3, $4, NOW(), NOW())",
            "fallback_key",
            "fallback_value",
            "test",
            1
        )
        .execute(&ctx.db_pool)
        .await
        .unwrap();
        
        // Should still work with database fallback
        let request = tonic::Request::new(GetConfigRequest {
            key: "fallback_key".to_string(),
            namespace: "test".to_string(),
        });
        
        let response = service.get_config(request).await;
        
        assert!(response.is_ok());
        let config = response.unwrap().into_inner().config.unwrap();
        assert_eq!(config.key, "fallback_key");
        assert_eq!(config.value, "fallback_value");
    }
}
```
import { describe, beforeEach, afterEach, it, expect, jest } from '@jest/globals';
import { ConfigStoreService } from '../../../src/config-store/config-store.service';
import { MockConfigRepository } from '../../mocks/config-repository.mock';
import { MockCacheService } from '../../mocks/cache-service.mock';
import { MockConfigValidator } from '../../mocks/config-validator.mock';
import { ConfigFactory } from '../../generators/config-factory';
import { ValidationError } from '../../../src/common/errors';

describe('ConfigStoreService', () => {
  let service: ConfigStoreService;
  let mockRepository: jest.Mocked<ConfigRepository>;
  let mockCache: jest.Mocked<CacheService>;
  let mockValidator: jest.Mocked<ConfigValidator>;
  let configFactory: ConfigFactory;

  beforeEach(() => {
    mockRepository = new MockConfigRepository() as jest.Mocked<ConfigRepository>;
    mockCache = new MockCacheService() as jest.Mocked<CacheService>;
    mockValidator = new MockConfigValidator() as jest.Mocked<ConfigValidator>;
    configFactory = new ConfigFactory({ seed: 42, realistic: true });
    
    service = new ConfigStoreService(mockRepository, mockCache, mockValidator);
  });

  afterEach(() => {
    jest.clearAllMocks();
  });

  describe('getConfig', () => {
    it('should return cached config when available', async () => {
      // Arrange
      const key = 'test-key';
      const namespace = 'trading';
      const cachedConfig = configFactory.generate({ key, namespace });
      const cacheKey = `config:${namespace}:${key}`;
      
      mockCache.get.mockResolvedValue(JSON.stringify(cachedConfig));

      // Act
      const result = await service.getConfig(key, namespace);

      // Assert
      expect(result).toEqual(cachedConfig);
      expect(mockCache.get).toHaveBeenCalledWith(cacheKey);
      expect(mockRepository.findByKey).not.toHaveBeenCalled();
    });

    it('should fetch from repository when cache miss occurs', async () => {
      // Arrange
      const key = 'test-key';
      const namespace = 'trading';
      const dbConfig = configFactory.generate({ key, namespace });
      const cacheKey = `config:${namespace}:${key}`;
      
      mockCache.get.mockResolvedValue(null); // Cache miss
      mockRepository.findByKey.mockResolvedValue(dbConfig);

      // Act
      const result = await service.getConfig(key, namespace);

      // Assert
      expect(result).toEqual(dbConfig);
      expect(mockCache.get).toHaveBeenCalledWith(cacheKey);
      expect(mockRepository.findByKey).toHaveBeenCalledWith(key, namespace);
      expect(mockCache.set).toHaveBeenCalledWith(
        cacheKey, 
        JSON.stringify(dbConfig), 
        300
      );
    });

    it('should return null when config not found', async () => {
      // Arrange
      const key = 'non-existent-key';
      const namespace = 'trading';
      
      mockCache.get.mockResolvedValue(null);
      mockRepository.findByKey.mockResolvedValue(null);

      // Act
      const result = await service.getConfig(key, namespace);

      // Assert
      expect(result).toBeNull();
      expect(mockCache.set).not.toHaveBeenCalled();
    });

    it('should use default namespace when not specified', async () => {
      // Arrange
      const key = 'test-key';
      const expectedConfig = configFactory.generate({ key, namespace: 'default' });
      
      mockCache.get.mockResolvedValue(null);
      mockRepository.findByKey.mockResolvedValue(expectedConfig);

      // Act
      const result = await service.getConfig(key);

      // Assert
      expect(result).toEqual(expectedConfig);
      expect(mockRepository.findByKey).toHaveBeenCalledWith(key, 'default');
    });

    it('should handle cache service errors gracefully', async () => {
      // Arrange
      const key = 'test-key';
      const namespace = 'trading';
      const dbConfig = configFactory.generate({ key, namespace });
      
      mockCache.get.mockRejectedValue(new Error('Cache service unavailable'));
      mockRepository.findByKey.mockResolvedValue(dbConfig);

      // Act
      const result = await service.getConfig(key, namespace);

      // Assert
      expect(result).toEqual(dbConfig);
      expect(mockRepository.findByKey).toHaveBeenCalledWith(key, namespace);
    });
  });

  describe('setConfig', () => {
    it('should create new config with valid data', async () => {
      // Arrange
      const key = 'max-position-size';
      const value = 1000000;
      const namespace = 'trading';
      const metadata = { description: 'Maximum position size in USD' };
      
      const expectedConfig = {
        key,
        value,
        namespace,
        version: 1,
        metadata,
        createdAt: expect.any(Date),
        updatedAt: expect.any(Date)
      };

      mockValidator.validate.mockResolvedValue({ isValid: true, errors: [] });
      mockRepository.findByKey.mockResolvedValue(null); // No existing config
      mockRepository.save.mockResolvedValue(expectedConfig as ConfigEntry);

      // Act
      const result = await service.setConfig(key, value, namespace, metadata);

      // Assert
      expect(result).toEqual(expectedConfig);
      expect(mockValidator.validate).toHaveBeenCalledWith(key, value, namespace);
      expect(mockRepository.save).toHaveBeenCalledWith(
        expect.objectContaining({
          key,
          value,
          namespace,
          version: 1,
          metadata
        })
      );
      expect(mockCache.set).toHaveBeenCalledWith(
        `config:${namespace}:${key}`,
        JSON.stringify(expectedConfig),
        300
      );
    });

    it('should increment version for existing config', async () => {
      // Arrange
      const key = 'max-position-size';
      const value = 2000000;
      const namespace = 'trading';
      const existingConfig = configFactory.generate({ 
        key, 
        namespace, 
        version: 5 
      });
      
      mockValidator.validate.mockResolvedValue({ isValid: true, errors: [] });
      mockRepository.findByKey.mockResolvedValue(existingConfig);
      mockRepository.save.mockResolvedValue({
        ...existingConfig,
        value,
        version: 6,
        updatedAt: new Date()
      });

      // Act
      const result = await service.setConfig(key, value, namespace);

      // Assert
      expect(result.version).toBe(6);
      expect(mockRepository.save).toHaveBeenCalledWith(
        expect.objectContaining({
          version: 6
        })
      );
    });

    it('should throw ValidationError for invalid config', async () => {
      // Arrange
      const key = 'max-position-size';
      const value = -1000; // Invalid negative value
      const namespace = 'trading';
      
      mockValidator.validate.mockResolvedValue({
        isValid: false,
        errors: ['Value must be positive']
      });

      // Act & Assert
      await expect(service.setConfig(key, value, namespace))
        .rejects
        .toThrow(ValidationError);
      
      expect(mockRepository.save).not.toHaveBeenCalled();
      expect(mockCache.set).not.toHaveBeenCalled();
    });

    it('should handle repository errors properly', async () => {
      // Arrange
      const key = 'test-key';
      const value = 'test-value';
      const namespace = 'trading';
      
      mockValidator.validate.mockResolvedValue({ isValid: true, errors: [] });
      mockRepository.findByKey.mockResolvedValue(null);
      mockRepository.save.mockRejectedValue(new Error('Database connection failed'));

      // Act & Assert
      await expect(service.setConfig(key, value, namespace))
        .rejects
        .toThrow('Database connection failed');
      
      expect(mockCache.set).not.toHaveBeenCalled();
    });
  });

  // Performance tests
  describe('Performance', () => {
    it('should complete getConfig within performance threshold', async () => {
      // Arrange
      const key = 'performance-test-key';
      const namespace = 'trading';
      const config = configFactory.generate({ key, namespace });
      const performanceThreshold = 50; // ms
      
      mockCache.get.mockResolvedValue(JSON.stringify(config));

      // Act
      const startTime = performance.now();
      await service.getConfig(key, namespace);
      const executionTime = performance.now() - startTime;

      // Assert
      expect(executionTime).toBeLessThan(performanceThreshold);
    });

    it('should handle concurrent getConfig requests efficiently', async () => {
      // Arrange
      const key = 'concurrent-test-key';
      const namespace = 'trading';
      const config = configFactory.generate({ key, namespace });
      const concurrentRequests = 100;
      
      mockCache.get.mockResolvedValue(JSON.stringify(config));

      // Act
      const promises = Array.from({ length: concurrentRequests }, () =>
        service.getConfig(key, namespace)
      );

      const startTime = performance.now();
      const results = await Promise.all(promises);
      const totalTime = performance.now() - startTime;

      // Assert
      expect(results).toHaveLength(concurrentRequests);
      results.forEach(result => {
        expect(result).toEqual(config);
      });
      
      // Average time per request should be reasonable
      const avgTimePerRequest = totalTime / concurrentRequests;
      expect(avgTimePerRequest).toBeLessThan(10); // ms
    });
  });

  // Edge cases
  describe('Edge Cases', () => {
    it('should handle extremely long configuration keys', async () => {
      // Arrange
      const longKey = 'x'.repeat(1000);
      const value = 'test-value';
      const namespace = 'trading';
      
      mockValidator.validate.mockResolvedValue({
        isValid: false,
        errors: ['Key exceeds maximum length']
      });

      // Act & Assert
      await expect(service.setConfig(longKey, value, namespace))
        .rejects
        .toThrow(ValidationError);
    });

    it('should handle configuration values with special characters', async () => {
      // Arrange
      const key = 'special-chars-key';
      const value = {
        message: 'Hello "World" & <script>alert("XSS")</script>',
        symbols: ['BTCUSD', 'ETH/USD', 'ADA-USD'],
        regex: '/^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$/'
      };
      const namespace = 'trading';
      
      mockValidator.validate.mockResolvedValue({ isValid: true, errors: [] });
      mockRepository.findByKey.mockResolvedValue(null);
      mockRepository.save.mockResolvedValue({
        key,
        value,
        namespace,
        version: 1,
        createdAt: new Date(),
        updatedAt: new Date(),
        metadata: {}
      });

      // Act
      const result = await service.setConfig(key, value, namespace);

      // Assert
      expect(result.value).toEqual(value);
      
      // Verify cache stores the complex value correctly
      const cacheCall = mockCache.set.mock.calls[0];
      const cachedValue = JSON.parse(cacheCall[1]);
      expect(cachedValue.value).toEqual(value);
    });

    it('should handle null and undefined values appropriately', async () => {
      // Arrange
      const key = 'nullable-key';
      const namespace = 'trading';
      
      // Test null value
      mockValidator.validate.mockResolvedValue({ isValid: true, errors: [] });
      mockRepository.findByKey.mockResolvedValue(null);
      mockRepository.save.mockResolvedValue({
        key,
        value: null,
        namespace,
        version: 1,
        createdAt: new Date(),
        updatedAt: new Date(),
        metadata: {}
      });

      // Act
      const nullResult = await service.setConfig(key, null, namespace);

      // Assert
      expect(nullResult.value).toBeNull();

      // Test undefined value (should be converted or rejected)
      mockValidator.validate.mockResolvedValue({
        isValid: false,
        errors: ['Value cannot be undefined']
      });

      await expect(service.setConfig(key, undefined, namespace))
        .rejects
        .toThrow(ValidationError);
    });
  });
});
```

### Integration Test Example
```typescript
// tests/integration/config-store/config-store.integration.test.ts
describe('ConfigStore Integration', () => {
  let app: Application;
  let database: TestDatabase;
  let cache: TestRedis;
  let configFactory: ConfigFactory;
  let authToken: string;

  beforeAll(async () => {
    database = TestDatabase.getInstance();
    await database.setupTestDatabase();
    
    cache = TestRedis.getInstance();
    await cache.connect();
    
    app = createTestApp({
      database: database.getConnection(),
      cache: cache.getConnection()
    });
    
    configFactory = new ConfigFactory();
    authToken = await createTestAuthToken();
  });

  afterAll(async () => {
    await app.close();
    await database.close();
    await cache.close();
  });

  beforeEach(async () => {
    await database.cleanDatabase();
    await cache.flushall();
  });

  describe('Config CRUD Operations', () => {
    it('should handle complete config lifecycle', async () => {
      const namespace = 'integration-test';
      const key = 'test-setting';
      const initialValue = { enabled: true, threshold: 100 };
      const updatedValue = { enabled: true, threshold: 200 };

      // Create config
      const createResponse = await request(app)
        .post('/api/config')
        .set('Authorization', `Bearer ${authToken}`)
        .send({
          key,
          value: initialValue,
          namespace,
          metadata: { description: 'Integration test setting' }
        })
        .expect(201);

      expect(createResponse.body).toMatchObject({
        key,
        value: initialValue,
        namespace,
        version: 1
      });

      // Verify config is stored in database
      const dbConfig = await database.query(
        'SELECT * FROM configs WHERE key = $1 AND namespace = $2',
        [key, namespace]
      );
      expect(dbConfig.rows).toHaveLength(1);
      expect(dbConfig.rows[0].value).toEqual(initialValue);

      // Get config (should be served from cache after creation)
      const getResponse = await request(app)
        .get(`/api/config/${namespace}/${key}`)
        .set('Authorization', `Bearer ${authToken}`)
        .expect(200);

      expect(getResponse.body).toMatchObject({
        key,
        value: initialValue,
        namespace,
        version: 1
      });

      // Update config
      const updateResponse = await request(app)
        .put(`/api/config/${namespace}/${key}`)
        .set('Authorization', `Bearer ${authToken}`)
        .send({
          value: updatedValue,
          metadata: { description: 'Updated integration test setting' }
        })
        .expect(200);

      expect(updateResponse.body).toMatchObject({
        key,
        value: updatedValue,
        namespace,
        version: 2
      });

      // Verify cache is updated
      const cacheKey = `config:${namespace}:${key}`;
      const cachedConfig = await cache.get(cacheKey);
      expect(JSON.parse(cachedConfig)).toMatchObject({
        value: updatedValue,
        version: 2
      });

      // Delete config
      await request(app)
        .delete(`/api/config/${namespace}/${key}`)
        .set('Authorization', `Bearer ${authToken}`)
        .expect(204);

      // Verify config is deleted
      await request(app)
        .get(`/api/config/${namespace}/${key}`)
        .set('Authorization', `Bearer ${authToken}`)
        .expect(404);

      // Verify cache is cleared
      const deletedCachedConfig = await cache.get(cacheKey);
      expect(deletedCachedConfig).toBeNull();
    });

    it('should handle configuration validation across service boundaries', async () => {
      const namespace = 'trading';
      const key = 'max-position-size';
      
      // Test valid configuration
      const validValue = 1000000;
      await request(app)
        .post('/api/config')
        .set('Authorization', `Bearer ${authToken}`)
        .send({
          key,
          value: validValue,
          namespace
        })
        .expect(201);

      // Test invalid configuration (negative value)
      const invalidValue = -1000;
      const errorResponse = await request(app)
        .post('/api/config')
        .set('Authorization', `Bearer ${authToken}`)
        .send({
          key: 'another-setting',
          value: invalidValue,
          namespace
        })
        .expect(400);

      expect(errorResponse.body).toHaveProperty('error');
      expect(errorResponse.body.error).toContain('must be positive');
    });
  });

  describe('Cache Behavior', () => {
    it('should use cache for subsequent reads', async () => {
      const namespace = 'cache-test';
      const key = 'cached-setting';
      const value = { setting: 'test-value' };

      // Create config
      await request(app)
        .post('/api/config')
        .set('Authorization', `Bearer ${authToken}`)
        .send({ key, value, namespace })
        .expect(201);

      // First read (from database)
      const startTime1 = performance.now();
      await request(app)
        .get(`/api/config/${namespace}/${key}`)
        .set('Authorization', `Bearer ${authToken}`)
        .expect(200);
      const firstReadTime = performance.now() - startTime1;

      // Second read (from cache) should be faster
      const startTime2 = performance.now();
      await request(app)
        .get(`/api/config/${namespace}/${key}`)
        .set('Authorization', `Bearer ${authToken}`)
        .expect(200);
      const secondReadTime = performance.now() - startTime2;

      // Cache read should be significantly faster
      expect(secondReadTime).toBeLessThan(firstReadTime * 0.5);
    });

    it('should handle cache failures gracefully', async () => {
      const namespace = 'cache-failure-test';
      const key = 'resilient-setting';
      const value = { resilience: 'test' };

      // Create config
      await request(app)
        .post('/api/config')
        .set('Authorization', `Bearer ${authToken}`)
        .send({ key, value, namespace })
        .expect(201);

      // Simulate cache failure
      await cache.disconnect();

      // Should still work (fall back to database)
      const response = await request(app)
        .get(`/api/config/${namespace}/${key}`)
        .set('Authorization', `Bearer ${authToken}`)
        .expect(200);

      expect(response.body.value).toEqual(value);

      // Reconnect cache for cleanup
      await cache.connect();
    });
  });

  describe('Configuration Events', () => {
    it('should publish events on configuration changes', async () => {
      const eventTracker = new EventTracker();
      app.on('config.changed', eventTracker.track.bind(eventTracker));

      const namespace = 'event-test';
      const key = 'event-setting';
      const value = { event: 'test-value' };

      // Create config (should trigger event)
      await request(app)
        .post('/api/config')
        .set('Authorization', `Bearer ${authToken}`)
        .send({ key, value, namespace })
        .expect(201);

      // Verify event was published
      await eventTracker.waitForEvent('config.changed', 1000);
      const events = eventTracker.getEvents('config.changed');
      
      expect(events).toHaveLength(1);
      expect(events[0]).toMatchObject({
        namespace,
        key,
        action: 'created',
        newValue: value
      });
    });
  });
});
```

## 2. Market Data Stream Testing Example

### Real-time Stream Testing
```typescript
// tests/integration/market-data/stream.integration.test.ts
describe('Market Data Stream Integration', () => {
  let mockMarketDataService: MockMarketDataService;
  let streamProcessor: MarketDataStreamProcessor;
  let testDataFactory: MarketDataFactory;
  let eventCollector: EventCollector;

  beforeAll(async () => {
    mockMarketDataService = new MockMarketDataService();
    await mockMarketDataService.start();
    
    streamProcessor = new MarketDataStreamProcessor({
      wsUrl: 'ws://localhost:8082',
      symbols: ['BTCUSD', 'ETHUSD'],
      reconnectInterval: 1000,
      heartbeatInterval: 30000
    });
    
    testDataFactory = new MarketDataFactory();
    eventCollector = new EventCollector();
  });

  afterAll(async () => {
    await streamProcessor.disconnect();
    await mockMarketDataService.stop();
  });

  beforeEach(async () => {
    eventCollector.clear();
    await streamProcessor.connect();
  });

  afterEach(async () => {
    await streamProcessor.disconnect();
  });

  describe('Real-time Data Processing', () => {
    it('should process market data updates in real-time', async () => {
      // Arrange
      const symbol = 'BTCUSD';
      const testData = testDataFactory.generateTimeSeries(
        symbol,
        new Date(),
        100, // 100ms intervals
        10   // 10 data points
      );

      streamProcessor.on('marketData', eventCollector.collect);
      await streamProcessor.subscribe([symbol]);

      // Act - Inject test data
      for (const dataPoint of testData) {
        mockMarketDataService.injectMarketData(symbol, dataPoint);
        await new Promise(resolve => setTimeout(resolve, 10));
      }

      // Wait for all data to be processed
      await eventCollector.waitForEvents('marketData', testData.length, 2000);

      // Assert
      const receivedData = eventCollector.getEvents('marketData');
      expect(receivedData).toHaveLength(testData.length);

      receivedData.forEach((received, index) => {
        expect(received.symbol).toBe(symbol);
        expect(received.price).toBeCloseTo(testData[index].price, 2);
        expect(received.timestamp).toBeInstanceOf(Date);
      });
    });

    it('should handle high-frequency data streams without dropping messages', async () => {
      // Arrange
      const symbol = 'BTCUSD';
      const messageCount = 1000;
      const frequency = 10; // ms between messages
      
      streamProcessor.on('marketData', eventCollector.collect);
      await streamProcessor.subscribe([symbol]);

      // Act - Generate high-frequency data
      const dataPromises = [];
      for (let i = 0; i < messageCount; i++) {
        const dataPoint = testDataFactory.generateForSymbol(symbol);
        dataPromises.push(
          new Promise(resolve => {
            setTimeout(() => {
              mockMarketDataService.injectMarketData(symbol, dataPoint);
              resolve(dataPoint);
            }, i * frequency);
          })
        );
      }

      await Promise.all(dataPromises);
      
      // Wait for all messages to be processed
      await eventCollector.waitForEvents('marketData', messageCount, 15000);

      // Assert
      const receivedMessages = eventCollector.getEvents('marketData');
      expect(receivedMessages).toHaveLength(messageCount);

      // Verify messages are in order (allowing for some timing variance)
      const timestamps = receivedMessages.map(msg => msg.timestamp.getTime());
      const sortedTimestamps = [...timestamps].sort();
      
      // Allow for small timing differences
      for (let i = 0; i < timestamps.length - 1; i++) {
        const timeDiff = Math.abs(timestamps[i] - sortedTimestamps[i]);
        expect(timeDiff).toBeLessThan(100); // 100ms tolerance
      }
    });

    it('should calculate accurate technical indicators from streaming data', async () => {
      // Arrange
      const symbol = 'BTCUSD';
      const basePrice = 50000;
      const priceMovements = [0, 100, -50, 200, -150, 300, -100]; // Price changes
      
      const indicatorCalculator = new TechnicalIndicatorCalculator();
      streamProcessor.on('marketData', (data) => {
        indicatorCalculator.update(data);
      });
      
      await streamProcessor.subscribe([symbol]);

      // Act - Send sequential price data
      let currentPrice = basePrice;
      for (const movement of priceMovements) {
        currentPrice += movement;
        const dataPoint = testDataFactory.generateForSymbol(symbol, currentPrice);
        mockMarketDataService.injectMarketData(symbol, dataPoint);
        await new Promise(resolve => setTimeout(resolve, 50));
      }

      await new Promise(resolve => setTimeout(resolve, 500)); // Allow processing

      // Assert
      const indicators = indicatorCalculator.getIndicators(symbol);
      
      // Verify SMA calculation
      expect(indicators.sma5).toBeCloseTo(
        priceMovements.slice(-5).reduce((sum, move, i, arr) => 
          sum + (basePrice + priceMovements.slice(0, priceMovements.length - arr.length + i + 1)
            .reduce((s, m) => s + m, 0))
        , 0) / 5,
        2
      );

      // Verify price volatility calculation
      expect(indicators.volatility).toBeGreaterThan(0);
      expect(indicators.volatility).toBeLessThan(1); // Should be reasonable
    });
  });

  describe('Connection Resilience', () => {
    it('should handle connection interruptions gracefully', async () => {
      // Arrange
      const symbol = 'BTCUSD';
      const reconnectEvents = [];
      
      streamProcessor.on('reconnected', () => reconnectEvents.push(new Date()));
      streamProcessor.on('marketData', eventCollector.collect);
      
      await streamProcessor.subscribe([symbol]);

      // Send initial data
      mockMarketDataService.injectMarketData(symbol, 
        testDataFactory.generateForSymbol(symbol)
      );
      await eventCollector.waitForEvents('marketData', 1, 1000);

      // Act - Simulate connection failure
      mockMarketDataService.simulateOutage(2000); // 2 second outage

      // Continue sending data during outage
      const dataPointsDuringOutage = 5;
      for (let i = 0; i < dataPointsDuringOutage; i++) {
        await new Promise(resolve => setTimeout(resolve, 100));
        mockMarketDataService.injectMarketData(symbol, 
          testDataFactory.generateForSymbol(symbol)
        );
      }

      // Wait for reconnection and data recovery
      await new Promise(resolve => setTimeout(resolve, 3000));

      // Assert
      expect(reconnectEvents.length).toBeGreaterThanOrEqual(1);
      
      // Should have received data after reconnection
      const totalEvents = eventCollector.getEvents('marketData');
      expect(totalEvents.length).toBeGreaterThan(1);
    });

    it('should implement exponential backoff for reconnection attempts', async () => {
      // Arrange
      const reconnectionAttempts = [];
      const originalConnect = streamProcessor.connect.bind(streamProcessor);
      
      streamProcessor.connect = jest.fn().mockImplementation(async () => {
        reconnectionAttempts.push({
          timestamp: Date.now(),
          attempt: reconnectionAttempts.length + 1
        });
        
        // Fail first few attempts
        if (reconnectionAttempts.length <= 3) {
          throw new Error('Connection failed');
        }
        
        return originalConnect();
      });

      // Act - Attempt to connect
      try {
        await streamProcessor.connect();
      } catch {
        // Expected to fail initially
      }

      // Wait for retry attempts
      await new Promise(resolve => setTimeout(resolve, 10000));

      // Assert
      expect(reconnectionAttempts.length).toBeGreaterThan(3);
      
      // Verify exponential backoff
      for (let i = 1; i < reconnectionAttempts.length - 1; i++) {
        const previousInterval = reconnectionAttempts[i].timestamp - reconnectionAttempts[i - 1].timestamp;
        const currentInterval = reconnectionAttempts[i + 1].timestamp - reconnectionAttempts[i].timestamp;
        
        // Each interval should be longer than the previous (with some tolerance)
        expect(currentInterval).toBeGreaterThanOrEqual(previousInterval * 0.8);
      }
    });
  });

  describe('Performance and Memory Management', () => {
    it('should handle sustained high-volume data without memory leaks', async () => {
      // Arrange
      const symbols = ['BTCUSD', 'ETHUSD', 'ADAUSD'];
      const testDuration = 30000; // 30 seconds
      const messagesPerSecond = 100;
      
      const initialMemory = process.memoryUsage().heapUsed;
      streamProcessor.on('marketData', eventCollector.collect);
      
      await streamProcessor.subscribe(symbols);

      // Act - Generate sustained high-volume data
      const startTime = Date.now();
      const dataInterval = setInterval(() => {
        symbols.forEach(symbol => {
          for (let i = 0; i < messagesPerSecond / symbols.length; i++) {
            mockMarketDataService.injectMarketData(symbol, 
              testDataFactory.generateForSymbol(symbol)
            );
          }
        });
      }, 1000);

      // Monitor memory usage during test
      const memorySnapshots = [];
      const memoryInterval = setInterval(() => {
        memorySnapshots.push({
          timestamp: Date.now() - startTime,
          heapUsed: process.memoryUsage().heapUsed
        });
      }, 5000);

      await new Promise(resolve => setTimeout(resolve, testDuration));
      
      clearInterval(dataInterval);
      clearInterval(memoryInterval);

      // Force garbage collection if available
      global.gc && global.gc();
      
      const finalMemory = process.memoryUsage().heapUsed;

      // Assert
      const memoryIncrease = finalMemory - initialMemory;
      const maxAcceptableIncrease = 100 * 1024 * 1024; // 100MB
      
      expect(memoryIncrease).toBeLessThan(maxAcceptableIncrease);
      
      // Memory should not continuously grow
      if (memorySnapshots.length >= 3) {
        const firstHalf = memorySnapshots.slice(0, Math.floor(memorySnapshots.length / 2));
        const secondHalf = memorySnapshots.slice(Math.floor(memorySnapshots.length / 2));
        
        const avgFirstHalf = firstHalf.reduce((sum, s) => sum + s.heapUsed, 0) / firstHalf.length;
        const avgSecondHalf = secondHalf.reduce((sum, s) => sum + s.heapUsed, 0) / secondHalf.length;
        
        // Memory growth should be bounded
        const growthRatio = avgSecondHalf / avgFirstHalf;
        expect(growthRatio).toBeLessThan(1.5); // Less than 50% growth
      }
    });
  });
});
```

This comprehensive testing strategy demonstrates real-world implementation of the Neural Trader V2 testing framework, showing how to test complex distributed systems with high reliability and performance requirements.