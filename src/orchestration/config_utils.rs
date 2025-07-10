//! Configuration utility functions for parsing connection URLs
//! 
//! This module provides utilities to parse Redis and PostgreSQL connection URLs
//! and extract connection parameters for adapter configuration.

use anyhow::{Context, Result};
use url::Url;
use crate::adapters::{
    redis::RedisConfig,
    timescale::TimescaleConfig,
};

/// Parse a Redis URL and extract connection parameters
/// 
/// # Arguments
/// 
/// * `redis_url` - Redis connection URL (e.g., "redis://[:password@]host:port[/db]")
/// 
/// # Returns
/// 
/// Returns a `RedisConfig` with parsed connection parameters
/// 
/// # Examples
/// 
/// ```rust
/// use neural_trader::orchestration::config_utils::parse_redis_url;
/// 
/// let config = parse_redis_url("redis://user:pass@localhost:6379/2").unwrap();
/// assert_eq!(config.host, "localhost");
/// assert_eq!(config.port, 6379);
/// assert_eq!(config.password, Some("pass".to_string()));
/// assert_eq!(config.db, 2);
/// ```
pub fn parse_redis_url(redis_url: &str) -> Result<RedisConfig> {
    let url = Url::parse(redis_url)
        .context("Failed to parse Redis URL")?;
    
    // Validate scheme
    if url.scheme() != "redis" && url.scheme() != "rediss" {
        anyhow::bail!("Invalid Redis URL scheme: expected 'redis' or 'rediss', got '{}'", url.scheme());
    }
    
    // Extract host
    let host = url.host_str()
        .ok_or_else(|| anyhow::anyhow!("Redis URL must contain a host"))?
        .to_string();
    
    // Extract port with default
    let port = url.port().unwrap_or(6379);
    
    // Extract password
    let password = if url.password().is_some() {
        Some(url.password().unwrap().to_string())
    } else {
        None
    };
    
    // Extract database number from path
    let db = if url.path().is_empty() || url.path() == "/" {
        0
    } else {
        let path = url.path().trim_start_matches('/');
        path.parse::<i64>()
            .context("Invalid Redis database number in URL path")?
    };
    
    Ok(RedisConfig {
        host,
        port,
        password,
        db,
        pool_size: 10, // Default pool size
    })
}

/// Parse a PostgreSQL URL and extract connection parameters
/// 
/// # Arguments
/// 
/// * `postgres_url` - PostgreSQL connection URL (e.g., "postgres://user:pass@host:port/database")
/// 
/// # Returns
/// 
/// Returns a `TimescaleConfig` with parsed connection parameters
/// 
/// # Examples
/// 
/// ```rust
/// use neural_trader::orchestration::config_utils::parse_postgres_url;
/// 
/// let config = parse_postgres_url("postgres://user:pass@localhost:5432/mydb").unwrap();
/// assert_eq!(config.host, "localhost");
/// assert_eq!(config.port, 5432);
/// assert_eq!(config.username, "user");
/// assert_eq!(config.password, "pass");
/// assert_eq!(config.database, "mydb");
/// ```
pub fn parse_postgres_url(postgres_url: &str) -> Result<TimescaleConfig> {
    let url = Url::parse(postgres_url)
        .context("Failed to parse PostgreSQL URL")?;
    
    // Validate scheme
    if url.scheme() != "postgres" && url.scheme() != "postgresql" {
        anyhow::bail!("Invalid PostgreSQL URL scheme: expected 'postgres' or 'postgresql', got '{}'", url.scheme());
    }
    
    // Extract host
    let host = url.host_str()
        .ok_or_else(|| anyhow::anyhow!("PostgreSQL URL must contain a host"))?
        .to_string();
    
    // Extract port with default
    let port = url.port().unwrap_or(5432);
    
    // Extract username
    let username = url.username().to_string();
    if username.is_empty() {
        anyhow::bail!("PostgreSQL URL must contain a username");
    }
    
    // Extract password
    let password = url.password()
        .ok_or_else(|| anyhow::anyhow!("PostgreSQL URL must contain a password"))?
        .to_string();
    
    // Extract database name from path
    let database = if url.path().is_empty() || url.path() == "/" {
        anyhow::bail!("PostgreSQL URL must contain a database name in the path");
    } else {
        url.path().trim_start_matches('/').to_string()
    };
    
    Ok(TimescaleConfig {
        host,
        port,
        database,
        username,
        password,
        max_connections: 10, // Default max connections
    })
}

/// Create a PostgreSQL connection URL from individual components
/// 
/// # Arguments
/// 
/// * `host` - Database host
/// * `port` - Database port
/// * `username` - Database username
/// * `password` - Database password
/// * `database` - Database name
/// 
/// # Returns
/// 
/// Returns a formatted PostgreSQL connection URL
pub fn build_postgres_url(host: &str, port: u16, username: &str, password: &str, database: &str) -> String {
    format!("postgres://{}:{}@{}:{}/{}", username, password, host, port, database)
}

/// Create a Redis connection URL from individual components
/// 
/// # Arguments
/// 
/// * `host` - Redis host
/// * `port` - Redis port
/// * `password` - Optional Redis password
/// * `db` - Redis database number
/// 
/// # Returns
/// 
/// Returns a formatted Redis connection URL
pub fn build_redis_url(host: &str, port: u16, password: Option<&str>, db: i64) -> String {
    if let Some(pass) = password {
        format!("redis://:{}@{}:{}/{}", pass, host, port, db)
    } else {
        format!("redis://{}:{}/{}", host, port, db)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_redis_url_basic() {
        let config = parse_redis_url("redis://localhost:6379").unwrap();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 6379);
        assert_eq!(config.password, None);
        assert_eq!(config.db, 0);
    }
    
    #[test]
    fn test_parse_redis_url_with_password() {
        let config = parse_redis_url("redis://:mypassword@localhost:6379/1").unwrap();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 6379);
        assert_eq!(config.password, Some("mypassword".to_string()));
        assert_eq!(config.db, 1);
    }
    
    #[test]
    fn test_parse_redis_url_with_default_port() {
        let config = parse_redis_url("redis://localhost").unwrap();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 6379);
        assert_eq!(config.password, None);
        assert_eq!(config.db, 0);
    }
    
    #[test]
    fn test_parse_postgres_url_basic() {
        let config = parse_postgres_url("postgres://user:pass@localhost:5432/mydb").unwrap();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 5432);
        assert_eq!(config.username, "user");
        assert_eq!(config.password, "pass");
        assert_eq!(config.database, "mydb");
    }
    
    #[test]
    fn test_parse_postgres_url_with_default_port() {
        let config = parse_postgres_url("postgres://user:pass@localhost/mydb").unwrap();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 5432);
        assert_eq!(config.username, "user");
        assert_eq!(config.password, "pass");
        assert_eq!(config.database, "mydb");
    }
    
    #[test]
    fn test_parse_postgres_url_postgresql_scheme() {
        let config = parse_postgres_url("postgresql://user:pass@localhost:5432/mydb").unwrap();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 5432);
        assert_eq!(config.username, "user");
        assert_eq!(config.password, "pass");
        assert_eq!(config.database, "mydb");
    }
    
    #[test]
    fn test_build_postgres_url() {
        let url = build_postgres_url("localhost", 5432, "user", "pass", "mydb");
        assert_eq!(url, "postgres://user:pass@localhost:5432/mydb");
    }
    
    #[test]
    fn test_build_redis_url_with_password() {
        let url = build_redis_url("localhost", 6379, Some("pass"), 1);
        assert_eq!(url, "redis://:pass@localhost:6379/1");
    }
    
    #[test]
    fn test_build_redis_url_without_password() {
        let url = build_redis_url("localhost", 6379, None, 0);
        assert_eq!(url, "redis://localhost:6379/0");
    }
    
    #[test]
    fn test_parse_invalid_redis_scheme() {
        assert!(parse_redis_url("http://localhost:6379").is_err());
    }
    
    #[test]
    fn test_parse_invalid_postgres_scheme() {
        assert!(parse_postgres_url("mysql://user:pass@localhost:5432/mydb").is_err());
    }
    
    #[test]
    fn test_parse_postgres_url_missing_username() {
        assert!(parse_postgres_url("postgres://:pass@localhost:5432/mydb").is_err());
    }
    
    #[test]
    fn test_parse_postgres_url_missing_password() {
        assert!(parse_postgres_url("postgres://user@localhost:5432/mydb").is_err());
    }
    
    #[test]
    fn test_parse_postgres_url_missing_database() {
        assert!(parse_postgres_url("postgres://user:pass@localhost:5432").is_err());
    }
}