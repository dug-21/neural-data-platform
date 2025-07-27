use anyhow::{Context, Result};
use redis::{aio::MultiplexedConnection, Client};
use serde::{Deserialize, Serialize};

/// Redis cache implementation for neural-trader
#[derive(Debug, Clone)]
pub struct RedisCache {
    pub conn: MultiplexedConnection,
}

/// Result of a prediction that can be cached
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PredictionResult {
    pub symbol: String,
    pub prediction: f64,
    pub confidence: f64,
    pub timestamp: i64,
}

impl RedisCache {
    /// Create a new Redis cache connection
    ///
    /// # Arguments
    /// * `redis_url` - Redis connection URL (e.g., "redis://127.0.0.1:6379")
    ///
    /// # Returns
    /// * `Result<Self>` - Redis cache instance or error
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = Client::open(redis_url)
            .context("Failed to create Redis client")?;
        
        let conn = client.get_multiplexed_async_connection().await
            .context("Failed to establish Redis connection")?;
        
        Ok(Self { conn })
    }
    
    /// Set a value in the cache with optional TTL
    ///
    /// # Arguments
    /// * `key` - Cache key
    /// * `value` - Value to cache (must be serializable)
    /// * `ttl_seconds` - Optional TTL in seconds
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    pub async fn set<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        ttl_seconds: Option<u64>,
    ) -> Result<()> {
        let serialized = serde_json::to_string(value)
            .context("Failed to serialize value")?;
        
        let mut conn = self.conn.clone();
        
        match ttl_seconds {
            Some(ttl) => {
                redis::cmd("SETEX")
                    .arg(key)
                    .arg(ttl)
                    .arg(serialized)
                    .query_async(&mut conn)
                    .await
                    .context("Failed to set value with TTL")?;
            }
            None => {
                redis::cmd("SET")
                    .arg(key)
                    .arg(serialized)
                    .query_async(&mut conn)
                    .await
                    .context("Failed to set value")?;
            }
        }
        
        Ok(())
    }
    
    /// Get a value from the cache
    ///
    /// # Arguments
    /// * `key` - Cache key
    ///
    /// # Returns
    /// * `Result<Option<T>>` - Cached value, None if not found, or error
    pub async fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Result<Option<T>> {
        let mut conn = self.conn.clone();
        
        let value: Option<String> = redis::cmd("GET")
            .arg(key)
            .query_async(&mut conn)
            .await
            .context("Failed to get value from Redis")?;
        
        match value {
            Some(data) => {
                match serde_json::from_str(&data) {
                    Ok(deserialized) => Ok(Some(deserialized)),
                    Err(e) => {
                        // Log the error but return None to handle gracefully
                        tracing::warn!("Failed to deserialize cached value for key '{}': {}", key, e);
                        Ok(None)
                    }
                }
            }
            None => Ok(None),
        }
    }
    
    /// Invalidate (delete) a key from the cache
    ///
    /// # Arguments
    /// * `key` - Cache key to invalidate
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    pub async fn invalidate(&self, key: &str) -> Result<()> {
        let mut conn = self.conn.clone();
        
        redis::cmd("DEL")
            .arg(key)
            .query_async(&mut conn)
            .await
            .context("Failed to invalidate key")?;
        
        Ok(())
    }
    
    /// Set a prediction result in the cache with TTL
    ///
    /// # Arguments
    /// * `key` - Cache key for the prediction
    /// * `prediction` - Prediction result to cache
    /// * `ttl` - TTL in seconds
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    pub async fn set_prediction(
        &self,
        key: &str,
        prediction: &PredictionResult,
        ttl: u64,
    ) -> Result<()> {
        self.set(key, prediction, Some(ttl)).await
            .context("Failed to cache prediction")
    }
    
    /// Get a prediction result from the cache
    ///
    /// # Arguments
    /// * `key` - Cache key for the prediction
    ///
    /// # Returns
    /// * `Result<Option<PredictionResult>>` - Cached prediction or None if not found
    pub async fn get_prediction(&self, key: &str) -> Result<Option<PredictionResult>> {
        self.get(key).await
            .context("Failed to retrieve prediction from cache")
    }
    
    /// Check if the cache is healthy by performing a PING
    ///
    /// # Returns
    /// * `Result<bool>` - True if healthy, false or error otherwise
    pub async fn health_check(&self) -> Result<bool> {
        let mut conn = self.conn.clone();
        
        let pong: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .context("Health check failed")?;
        
        Ok(pong == "PONG")
    }
    
    /// Get remaining TTL for a key in seconds
    ///
    /// # Arguments
    /// * `key` - Cache key
    ///
    /// # Returns
    /// * `Result<Option<i64>>` - TTL in seconds, None if key doesn't exist or has no TTL
    pub async fn get_ttl(&self, key: &str) -> Result<Option<i64>> {
        let mut conn = self.conn.clone();
        
        let ttl: i64 = redis::cmd("TTL")
            .arg(key)
            .query_async(&mut conn)
            .await
            .context("Failed to get TTL")?;
        
        match ttl {
            -2 => Ok(None), // Key does not exist
            -1 => Ok(Some(-1)), // Key exists but has no TTL
            _ => Ok(Some(ttl)), // TTL in seconds
        }
    }
    
    /// Set multiple key-value pairs in a single operation
    ///
    /// # Arguments
    /// * `items` - Vector of (key, value, optional_ttl) tuples
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    pub async fn set_multiple<T: Serialize>(
        &self,
        items: Vec<(&str, &T, Option<u64>)>,
    ) -> Result<()> {
        for (key, value, ttl) in items {
            self.set(key, value, ttl).await
                .with_context(|| format!("Failed to set key: {}", key))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_prediction_result_serialization() {
        let prediction = PredictionResult {
            symbol: "BTCUSD".to_string(),
            prediction: 45000.0,
            confidence: 0.85,
            timestamp: 1234567890,
        };
        
        // Test serialization
        let serialized = serde_json::to_string(&prediction).unwrap();
        assert!(serialized.contains("BTCUSD"));
        assert!(serialized.contains("45000"));
        
        // Test deserialization
        let deserialized: PredictionResult = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.symbol, prediction.symbol);
        assert_eq!(deserialized.prediction, prediction.prediction);
        assert_eq!(deserialized.confidence, prediction.confidence);
        assert_eq!(deserialized.timestamp, prediction.timestamp);
    }
}