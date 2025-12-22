//! Database configuration module
//!
//! Handles database connection and persistence configuration.

use serde::{Deserialize, Serialize};

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout: u64,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout: u64,
    #[serde(default = "default_max_query_time")]
    pub max_query_time: u64,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgresql://localhost/neural_trader".to_string(),
            max_connections: 10,
            min_connections: 2,
            connection_timeout: default_connection_timeout(),
            idle_timeout: default_idle_timeout(),
            max_query_time: default_max_query_time(),
        }
    }
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub max_connections: u32,
    pub default_ttl_seconds: u64,
    #[serde(default = "default_redis_connection_timeout_ms")]
    pub connection_timeout_ms: u64,
    #[serde(default = "default_false")]
    pub cluster_mode: bool,
    #[serde(default = "default_redis_pool_max_idle")]
    pub pool_max_idle: u32,
    #[serde(default = "default_redis_pool_timeout_seconds")]
    pub pool_timeout_seconds: u64,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            max_connections: 10,
            default_ttl_seconds: 3600,
            connection_timeout_ms: default_redis_connection_timeout_ms(),
            cluster_mode: default_false(),
            pool_max_idle: default_redis_pool_max_idle(),
            pool_timeout_seconds: default_redis_pool_timeout_seconds(),
        }
    }
}

// Default value functions
fn default_connection_timeout() -> u64 {
    30
}
fn default_idle_timeout() -> u64 {
    600
}
fn default_max_query_time() -> u64 {
    30
}
fn default_redis_connection_timeout_ms() -> u64 {
    5000
}
fn default_false() -> bool {
    false
}
fn default_redis_pool_max_idle() -> u32 {
    8
}
fn default_redis_pool_timeout_seconds() -> u64 {
    30
}

/// Backup configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    #[serde(default = "default_backup_enabled")]
    pub enabled: bool,
    #[serde(default = "default_backup_interval_hours")]
    pub interval_hours: u64,
    #[serde(default = "default_backup_retention_days")]
    pub retention_days: u32,
    #[serde(default = "default_backup_path")]
    pub backup_path: String,
    #[serde(default = "default_backup_compression")]
    pub compression: bool,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            enabled: default_backup_enabled(),
            interval_hours: default_backup_interval_hours(),
            retention_days: default_backup_retention_days(),
            backup_path: default_backup_path(),
            compression: default_backup_compression(),
        }
    }
}

fn default_backup_enabled() -> bool {
    true
}
fn default_backup_interval_hours() -> u64 {
    24
}
fn default_backup_retention_days() -> u32 {
    30
}
fn default_backup_path() -> String {
    "./backups".to_string()
}
fn default_backup_compression() -> bool {
    true
}
