//! Configuration loading for Silver ETL
//!
//! Loads stream configurations with silver_etl sections from etcd or files.
//! Priority: etcd -> YAML files (fallback)

use anyhow::{Context, Result};
use config_client::{ConfigClient, StreamRegistry};
use neural_core::config::SilverEtlConfig;
use std::path::Path;
use tracing::{debug, info};

/// Configuration loader for Silver ETL
///
/// Loads stream configs from etcd (preferred) or YAML files (fallback).
/// Extracts the `silver_etl` section from stream configurations.
pub struct ConfigLoader {
    etcd_endpoint: String,
    config_dir: String,
}

impl ConfigLoader {
    /// Create a new ConfigLoader with the given etcd endpoint and config directory
    pub fn new(etcd_endpoint: &str, config_dir: &str) -> Self {
        Self {
            etcd_endpoint: etcd_endpoint.to_string(),
            config_dir: config_dir.to_string(),
        }
    }

    /// Load silver ETL config for a specific stream
    ///
    /// Tries etcd first, falls back to YAML file if etcd unavailable.
    pub async fn load_stream_config(&self, stream_id: &str) -> Result<SilverEtlConfig> {
        debug!(stream_id = %stream_id, "Loading stream config");

        // Try etcd first
        match self.load_from_etcd(stream_id).await {
            Ok(config) => {
                debug!("Loaded from etcd successfully");
                return Ok(config);
            }
            Err(e) => {
                debug!(error = %e, "etcd failed, falling back to YAML");
            }
        }

        // Fallback to YAML
        self.load_from_yaml(stream_id).await
    }

    /// Load config from etcd
    ///
    /// Fetches silver_etl config from flattened etcd keys under /streams/{stream_id}/silver_etl/
    /// Keys like /silver_etl/enabled, /silver_etl/target_table, etc. are unflattened into
    /// a nested JSON object and deserialized to SilverEtlConfig.
    async fn load_from_etcd(&self, stream_id: &str) -> Result<SilverEtlConfig> {
        // First verify the stream exists in etcd (validates stream_id)
        let registry = StreamRegistry::new(&[&self.etcd_endpoint])
            .await
            .context("Failed to connect to etcd")?;

        // Check if stream exists - this validates the stream_id is known
        let stream_exists = registry
            .stream_exists(stream_id)
            .await
            .context(format!("Failed to check stream '{}' in etcd", stream_id))?;

        if !stream_exists {
            return Err(anyhow::anyhow!("Stream '{}' not found in etcd", stream_id));
        }

        // Fetch silver_etl config using get_prefix_nested to unflatten keys
        let client = ConfigClient::new(&[&self.etcd_endpoint])
            .await
            .context("Failed to connect to etcd")?;

        let prefix = format!("/streams/{}/silver_etl", stream_id);
        let nested_value = client
            .get_prefix_nested(&prefix)
            .await
            .context(format!(
                "Stream '{}' has no silver_etl config in etcd (run sync script)",
                stream_id
            ))?;

        let config: SilverEtlConfig = serde_json::from_value(nested_value).context(format!(
            "Failed to deserialize silver_etl config for stream '{}'",
            stream_id
        ))?;

        info!(stream_id = %stream_id, "Loaded silver_etl config from etcd");
        Ok(config)
    }

    /// Load config from YAML file
    ///
    /// Tries both directory structure (stream_id/config.yaml) and flat (stream_id.yaml)
    async fn load_from_yaml(&self, stream_id: &str) -> Result<SilverEtlConfig> {
        // Try directory structure first: {config_dir}/{stream_id}/config.yaml
        let dir_path = Path::new(&self.config_dir).join(stream_id).join("config.yaml");
        // Then try flat structure: {config_dir}/{stream_id}.yaml
        let flat_path = Path::new(&self.config_dir).join(format!("{}.yaml", stream_id));

        let yaml_path = if dir_path.exists() {
            dir_path
        } else if flat_path.exists() {
            flat_path
        } else {
            return Err(anyhow::anyhow!(
                "Config file not found: tried {} and {} (neither in etcd nor YAML)",
                dir_path.display(),
                flat_path.display()
            ));
        };

        debug!(path = %yaml_path.display(), "Loading config from YAML");

        let contents = tokio::fs::read_to_string(&yaml_path)
            .await
            .context(format!("Failed to read {}", yaml_path.display()))?;

        let stream_config: StreamConfigWithSilver = serde_yaml::from_str(&contents)
            .context(format!("Failed to parse {}", yaml_path.display()))?;

        info!(stream_id = %stream_id, path = %yaml_path.display(), "Loaded config from YAML");

        stream_config.silver_etl.ok_or_else(|| {
            anyhow::anyhow!("Stream '{}' has no silver_etl section in YAML", stream_id)
        })
    }

    /// Load all streams with silver_etl.enabled = true
    pub async fn load_all_enabled(&self) -> Result<Vec<String>> {
        let mut enabled_streams = Vec::new();

        // Get all streams
        let all_streams = self.list_all_streams().await?;

        for stream_id in all_streams {
            match self.load_stream_config(&stream_id).await {
                Ok(config) if config.enabled => {
                    debug!(stream_id = %stream_id, "Found enabled stream");
                    enabled_streams.push(stream_id);
                }
                Ok(_) => {
                    debug!(stream_id = %stream_id, "Stream has silver_etl disabled");
                }
                Err(e) => {
                    debug!(stream_id = %stream_id, error = %e, "Stream has no silver_etl config");
                }
            }
        }

        Ok(enabled_streams)
    }

    /// List all available streams (from etcd or YAML files)
    pub async fn list_all_streams(&self) -> Result<Vec<String>> {
        // Try etcd first
        if let Ok(streams) = self.list_streams_from_etcd().await {
            if !streams.is_empty() {
                return Ok(streams);
            }
        }

        // Fallback to YAML directory
        self.list_streams_from_yaml().await
    }

    /// List streams from etcd
    async fn list_streams_from_etcd(&self) -> Result<Vec<String>> {
        let registry = StreamRegistry::new(&[&self.etcd_endpoint])
            .await
            .context("Failed to connect to etcd")?;

        let streams = registry
            .list_streams()
            .await
            .context("Failed to list streams from etcd")?;

        Ok(streams)
    }

    /// List streams from YAML directory
    async fn list_streams_from_yaml(&self) -> Result<Vec<String>> {
        let config_path = Path::new(&self.config_dir);

        if !config_path.exists() {
            return Ok(Vec::new());
        }

        let mut streams = Vec::new();
        let mut entries = tokio::fs::read_dir(config_path)
            .await
            .context(format!("Failed to read directory {}", self.config_dir))?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path
                .extension()
                .map_or(false, |ext| ext == "yaml" || ext == "yml")
            {
                if let Some(stem) = path.file_stem() {
                    streams.push(stem.to_string_lossy().to_string());
                }
            }
        }

        streams.sort();
        Ok(streams)
    }
}

/// Intermediate struct to extract silver_etl from full stream config
#[derive(Debug, serde::Deserialize)]
struct StreamConfigWithSilver {
    #[serde(default)]
    silver_etl: Option<SilverEtlConfig>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_load_from_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
stream_id: test-stream
silver_etl:
  enabled: true
  target_table: silver.test_observations
  timestamp:
    source_field: timestamp
    target_field: observation_time
    transform: microseconds_to_timestamp
  field_mappings: []
"#;

        let config_path = temp_dir.path().join("test-stream.yaml");
        tokio::fs::write(&config_path, config_content)
            .await
            .unwrap();

        let loader = ConfigLoader::new("http://localhost:2379", temp_dir.path().to_str().unwrap());

        let config = loader.load_from_yaml("test-stream").await.unwrap();
        assert!(config.enabled);
        assert_eq!(config.target_table, "silver.test_observations");
    }

    #[tokio::test]
    async fn test_list_streams_from_yaml() {
        let temp_dir = TempDir::new().unwrap();

        // Create test YAML files
        tokio::fs::write(
            temp_dir.path().join("air-quality.yaml"),
            "stream_id: air-quality",
        )
        .await
        .unwrap();
        tokio::fs::write(
            temp_dir.path().join("outdoor-weather.yaml"),
            "stream_id: outdoor-weather",
        )
        .await
        .unwrap();
        tokio::fs::write(temp_dir.path().join("not-yaml.txt"), "ignore me")
            .await
            .unwrap();

        let loader = ConfigLoader::new("http://localhost:2379", temp_dir.path().to_str().unwrap());

        let streams = loader.list_streams_from_yaml().await.unwrap();
        assert_eq!(streams.len(), 2);
        assert!(streams.contains(&"air-quality".to_string()));
        assert!(streams.contains(&"outdoor-weather".to_string()));
    }

    #[tokio::test]
    async fn test_missing_yaml_file() {
        let temp_dir = TempDir::new().unwrap();

        let loader = ConfigLoader::new("http://localhost:2379", temp_dir.path().to_str().unwrap());

        let result = loader.load_from_yaml("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_yaml_without_silver_etl_section() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
stream_id: test-stream
mqtt:
  topic: test/topic
"#;

        let config_path = temp_dir.path().join("test-stream.yaml");
        tokio::fs::write(&config_path, config_content)
            .await
            .unwrap();

        let loader = ConfigLoader::new("http://localhost:2379", temp_dir.path().to_str().unwrap());

        let result = loader.load_from_yaml("test-stream").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("no silver_etl section"));
    }

    #[tokio::test]
    async fn test_config_loader_creation() {
        let loader = ConfigLoader::new("http://etcd:2379", "/config/streams");
        assert_eq!(loader.etcd_endpoint, "http://etcd:2379");
        assert_eq!(loader.config_dir, "/config/streams");
    }
}
