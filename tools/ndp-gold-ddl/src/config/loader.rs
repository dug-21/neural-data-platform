//! Configuration loading for Gold DDL generation
//!
//! Loads stream configurations from the file system.

use crate::config::domain::DomainConfig;
use crate::config::types::StreamConfig;
use crate::error::{GoldDdlError, Result};
use std::path::{Path, PathBuf};

/// Trait for loading configurations
pub trait ConfigLoader: Send + Sync {
    /// Load stream configuration by stream ID
    fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig>;

    /// Load domain configuration by domain ID
    fn load_domain_config(&self, domain_id: &str) -> Result<DomainConfig>;
}

/// File system configuration loader
pub struct FileSystemConfigLoader {
    config_dir: PathBuf,
}

impl FileSystemConfigLoader {
    /// Create a new loader with the given config directory
    pub fn new(config_dir: impl Into<PathBuf>) -> Self {
        Self {
            config_dir: config_dir.into(),
        }
    }

    /// Get the path to a stream's config file
    fn stream_config_path(&self, stream_id: &str) -> PathBuf {
        self.config_dir
            .join("base")
            .join("streams")
            .join(stream_id)
            .join("config.json")
    }

    /// Get the path to a domain's config file
    fn domain_config_path(&self, domain_id: &str) -> PathBuf {
        self.config_dir
            .join("domains")
            .join(domain_id)
            .join("domain.yaml")
    }
}

impl ConfigLoader for FileSystemConfigLoader {
    fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig> {
        let path = self.stream_config_path(stream_id);

        if !path.exists() {
            return Err(GoldDdlError::ConfigNotFound {
                path: path.display().to_string(),
            });
        }

        let content = std::fs::read_to_string(&path)?;
        let config: StreamConfig =
            serde_json::from_str(&content).map_err(|e| GoldDdlError::ConfigParseError {
                message: format!("Failed to parse {}: {}", path.display(), e),
            })?;

        Ok(config)
    }

    fn load_domain_config(&self, domain_id: &str) -> Result<DomainConfig> {
        let path = self.domain_config_path(domain_id);

        if !path.exists() {
            return Err(GoldDdlError::ConfigNotFound {
                path: path.display().to_string(),
            });
        }

        let content = std::fs::read_to_string(&path)?;
        let config: DomainConfig =
            serde_yaml::from_str(&content).map_err(|e| GoldDdlError::ConfigParseError {
                message: format!("Failed to parse {}: {}", path.display(), e),
            })?;

        Ok(config)
    }
}

/// Create a default config loader for the given config directory
pub fn default_loader(config_dir: impl Into<PathBuf>) -> impl ConfigLoader {
    FileSystemConfigLoader::new(config_dir)
}

/// Resolve the config directory path
pub fn resolve_config_dir(config_dir: Option<&Path>) -> PathBuf {
    config_dir.map(PathBuf::from).unwrap_or_else(|| {
        // Check for Pi deployment path first
        let pi_path = PathBuf::from("/opt/ndp/config");
        if pi_path.exists() {
            pi_path
        } else {
            // Default to local development path
            PathBuf::from("./config")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_config(dir: &Path, stream_id: &str, content: &str) {
        let stream_dir = dir.join("base").join("streams").join(stream_id);
        std::fs::create_dir_all(&stream_dir).unwrap();
        let config_path = stream_dir.join("config.json");
        let mut file = std::fs::File::create(config_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_load_stream_config_success() {
        let temp_dir = TempDir::new().unwrap();
        let config_json = r#"{
            "stream_id": "air-quality",
            "fields": [
                { "name": "pm25", "type": "float" },
                { "name": "co2", "type": "int" }
            ],
            "silver_etl": {
                "target_table": "silver.air_quality_observations"
            },
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["1 hour"],
                    "fields": {
                        "pm25": { "metrics": ["mean", "std"] }
                    }
                }
            }
        }"#;

        create_test_config(temp_dir.path(), "air-quality", config_json);

        let loader = FileSystemConfigLoader::new(temp_dir.path());
        let config = loader.load_stream_config("air-quality").unwrap();

        assert_eq!(config.stream_id, "air-quality");
        assert_eq!(config.fields.len(), 2);
        assert!(config.gold_etl.is_some());
        assert!(config.gold_etl.unwrap().enabled);
    }

    #[test]
    fn test_load_stream_config_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let loader = FileSystemConfigLoader::new(temp_dir.path());

        let result = loader.load_stream_config("nonexistent");

        assert!(result.is_err());
        match result.unwrap_err() {
            GoldDdlError::ConfigNotFound { path } => {
                assert!(path.contains("nonexistent"));
            }
            _ => panic!("Expected ConfigNotFound error"),
        }
    }

    #[test]
    fn test_load_stream_config_parse_error() {
        let temp_dir = TempDir::new().unwrap();
        create_test_config(temp_dir.path(), "bad-config", "{ invalid json }");

        let loader = FileSystemConfigLoader::new(temp_dir.path());
        let result = loader.load_stream_config("bad-config");

        assert!(result.is_err());
        match result.unwrap_err() {
            GoldDdlError::ConfigParseError { message } => {
                assert!(message.contains("bad-config"));
            }
            _ => panic!("Expected ConfigParseError"),
        }
    }
}
