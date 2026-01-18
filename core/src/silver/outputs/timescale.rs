//! TimescaleDB output implementation for Silver layer (stub)
//!
//! This provides a stub implementation. Full implementation requires sqlx.

use super::{SilverOutput, SilverOutputError};
use crate::silver::types::SilverRecord;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for TimescaleDB output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimescaleConfig {
    pub connection_string: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    #[serde(default = "default_table")]
    pub default_table: String,
    #[serde(default)]
    pub table_mapping: HashMap<String, String>,
}

fn default_max_connections() -> u32 {
    5
}

fn default_table() -> String {
    "silver.observations".to_string()
}

impl Default for TimescaleConfig {
    fn default() -> Self {
        Self {
            connection_string: String::new(),
            max_connections: default_max_connections(),
            default_table: default_table(),
            table_mapping: HashMap::new(),
        }
    }
}

/// TimescaleDB output sink (stub implementation)
pub struct TimescaleOutput {
    config: TimescaleConfig,
}

impl TimescaleOutput {
    pub async fn new(config: TimescaleConfig) -> Result<Self, SilverOutputError> {
        if config.connection_string.is_empty() {
            return Err(SilverOutputError::ConfigError(
                "connection_string is required".to_string(),
            ));
        }
        Ok(Self { config })
    }

    fn get_table(&self, record: &SilverRecord) -> String {
        self.config
            .table_mapping
            .get(&record.stream_id)
            .cloned()
            .unwrap_or_else(|| self.config.default_table.clone())
    }
}

#[async_trait]
impl SilverOutput for TimescaleOutput {
    async fn write(&self, record: &SilverRecord) -> Result<(), SilverOutputError> {
        if record.should_drop() {
            return Ok(());
        }
        let _table = self.get_table(record);
        // Stub: would execute INSERT here
        Err(SilverOutputError::WriteError(
            "TimescaleOutput requires sqlx feature".to_string(),
        ))
    }

    async fn get_watermark(
        &self,
        _stream_id: &str,
    ) -> Result<Option<DateTime<Utc>>, SilverOutputError> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<bool, SilverOutputError> {
        Ok(!self.config.connection_string.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timescale_config_default() {
        let config = TimescaleConfig::default();
        assert_eq!(config.max_connections, 5);
        assert!(config.connection_string.is_empty());
    }

    #[tokio::test]
    async fn test_new_requires_connection_string() {
        let config = TimescaleConfig::default();
        let result = TimescaleOutput::new(config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_new_with_valid_config() {
        let config = TimescaleConfig {
            connection_string: "postgresql://localhost/test".to_string(),
            ..Default::default()
        };
        let result = TimescaleOutput::new(config).await;
        assert!(result.is_ok());
    }
}
