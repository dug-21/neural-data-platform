use crate::error::{Error, Result};
use redis::{AsyncCommands, Client, aio::MultiplexedConnection};
use serde::{Serialize, Deserialize};
use std::time::Duration;
use tracing::{info, error, debug};

#[derive(Clone)]
pub struct RedisClient {
    conn: MultiplexedConnection,
}

impl RedisClient {
    pub async fn new(redis_url: &str) -> Result<Self> {
        info!("Connecting to Redis...");
        
        let client = Client::open(redis_url)
            .map_err(|e| Error::Redis(e))?;
        
        let conn = client.get_multiplexed_async_connection().await
            .map_err(|e| Error::Redis(e))?;
        
        info!("Redis connection established");
        
        Ok(Self { conn })
    }
    
    pub async fn get<T: for<'de> Deserialize<'de>>(&mut self, key: &str) -> Result<Option<T>> {
        debug!("Redis GET: {}", key);
        
        let value: Option<String> = self.conn.get(key).await
            .map_err(|e| Error::Redis(e))?;
        
        match value {
            Some(json_str) => {
                let data = serde_json::from_str(&json_str)
                    .map_err(|e| Error::Serialization(e))?;
                Ok(Some(data))
            }
            None => Ok(None),
        }
    }
    
    pub async fn set<T: Serialize>(&mut self, key: &str, value: &T, ttl: Duration) -> Result<()> {
        debug!("Redis SET: {} (TTL: {:?})", key, ttl);
        
        let json_str = serde_json::to_string(value)
            .map_err(|e| Error::Serialization(e))?;
        
        self.conn.set_ex(key, json_str, ttl.as_secs()).await
            .map_err(|e| Error::Redis(e))?;
        
        Ok(())
    }
    
    pub async fn delete(&mut self, key: &str) -> Result<()> {
        debug!("Redis DEL: {}", key);
        
        self.conn.del(key).await
            .map_err(|e| Error::Redis(e))?;
        
        Ok(())
    }
    
    pub async fn exists(&mut self, key: &str) -> Result<bool> {
        debug!("Redis EXISTS: {}", key);
        
        let exists: bool = self.conn.exists(key).await
            .map_err(|e| Error::Redis(e))?;
        
        Ok(exists)
    }
    
    pub async fn expire(&mut self, key: &str, ttl: Duration) -> Result<()> {
        debug!("Redis EXPIRE: {} (TTL: {:?})", key, ttl);
        
        self.conn.expire(key, ttl.as_secs() as i64).await
            .map_err(|e| Error::Redis(e))?;
        
        Ok(())
    }
    
    pub async fn get_ttl(&mut self, key: &str) -> Result<Option<Duration>> {
        debug!("Redis TTL: {}", key);
        
        let ttl: i64 = self.conn.ttl(key).await
            .map_err(|e| Error::Redis(e))?;
        
        if ttl < 0 {
            Ok(None)
        } else {
            Ok(Some(Duration::from_secs(ttl as u64)))
        }
    }
    
    pub async fn hget<T: for<'de> Deserialize<'de>>(&mut self, key: &str, field: &str) -> Result<Option<T>> {
        debug!("Redis HGET: {} {}", key, field);
        
        let value: Option<String> = self.conn.hget(key, field).await
            .map_err(|e| Error::Redis(e))?;
        
        match value {
            Some(json_str) => {
                let data = serde_json::from_str(&json_str)
                    .map_err(|e| Error::Serialization(e))?;
                Ok(Some(data))
            }
            None => Ok(None),
        }
    }
    
    pub async fn hset<T: Serialize>(&mut self, key: &str, field: &str, value: &T) -> Result<()> {
        debug!("Redis HSET: {} {}", key, field);
        
        let json_str = serde_json::to_string(value)
            .map_err(|e| Error::Serialization(e))?;
        
        self.conn.hset(key, field, json_str).await
            .map_err(|e| Error::Redis(e))?;
        
        Ok(())
    }
    
    pub async fn hgetall(&mut self, key: &str) -> Result<std::collections::HashMap<String, String>> {
        debug!("Redis HGETALL: {}", key);
        
        let map: std::collections::HashMap<String, String> = self.conn.hgetall(key).await
            .map_err(|e| Error::Redis(e))?;
        
        Ok(map)
    }
    
    pub async fn keys(&mut self, pattern: &str) -> Result<Vec<String>> {
        debug!("Redis KEYS: {}", pattern);
        
        let keys: Vec<String> = self.conn.keys(pattern).await
            .map_err(|e| Error::Redis(e))?;
        
        Ok(keys)
    }
    
    pub async fn incr(&mut self, key: &str) -> Result<i64> {
        debug!("Redis INCR: {}", key);
        
        let value: i64 = self.conn.incr(key, 1).await
            .map_err(|e| Error::Redis(e))?;
        
        Ok(value)
    }
    
    pub async fn decr(&mut self, key: &str) -> Result<i64> {
        debug!("Redis DECR: {}", key);
        
        let value: i64 = self.conn.decr(key, 1).await
            .map_err(|e| Error::Redis(e))?;
        
        Ok(value)
    }
}