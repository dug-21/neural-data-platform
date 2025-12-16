use config_client::stream::registry::StreamRegistry;
use config_client::ConfigError;
use neural_core::{StreamConfig, StreamConfigError};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Errors that can occur during config sync operations
#[derive(Debug, Error)]
pub enum ConfigSyncError {
    #[error("Failed to read YAML file: {0}")]
    YamlReadError(String),

    #[error("Failed to parse YAML: {0}")]
    YamlParseError(String),

    #[error("Invalid stream configuration: {0}")]
    InvalidConfig(String),

    #[error("Registry operation failed: {0}")]
    RegistryError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Directory not found: {0}")]
    DirectoryNotFound(String),
}

impl From<serde_yaml::Error> for ConfigSyncError {
    fn from(e: serde_yaml::Error) -> Self {
        ConfigSyncError::YamlParseError(e.to_string())
    }
}

impl From<ConfigError> for ConfigSyncError {
    fn from(e: ConfigError) -> Self {
        ConfigSyncError::RegistryError(e.to_string())
    }
}

impl From<StreamConfigError> for ConfigSyncError {
    fn from(e: StreamConfigError) -> Self {
        ConfigSyncError::InvalidConfig(e.to_string())
    }
}

/// Report of sync operation results
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyncReport {
    /// Stream IDs successfully synced
    pub synced: Vec<String>,

    /// Stream IDs that failed with error messages
    pub failed: Vec<(String, String)>,

    /// Stream IDs that were skipped (disabled or unchanged)
    pub skipped: Vec<String>,
}

impl SyncReport {
    /// Create a new empty sync report
    pub fn new() -> Self {
        Self {
            synced: Vec::new(),
            failed: Vec::new(),
            skipped: Vec::new(),
        }
    }

    /// Get total number of configs processed
    pub fn total(&self) -> usize {
        self.synced.len() + self.failed.len() + self.skipped.len()
    }

    /// Check if any syncs failed
    pub fn has_failures(&self) -> bool {
        !self.failed.is_empty()
    }

    /// Check if all syncs succeeded
    pub fn is_success(&self) -> bool {
        self.failed.is_empty()
    }
}

impl Default for SyncReport {
    fn default() -> Self {
        Self::new()
    }
}

/// ConfigSyncService handles synchronization of YAML configs to StreamRegistry
pub struct ConfigSyncService {
    config_dir: PathBuf,
}

impl ConfigSyncService {
    /// Create a new ConfigSyncService with the specified config directory
    pub fn new(config_dir: impl AsRef<Path>) -> Self {
        info!("Initializing ConfigSyncService with config_dir: {:?}", config_dir.as_ref());
        Self {
            config_dir: config_dir.as_ref().to_path_buf(),
        }
    }

    /// Load a single stream config from a YAML file
    pub async fn load_yaml_config(&self, yaml_path: impl AsRef<Path>) -> Result<StreamConfig, ConfigSyncError> {
        let yaml_path = yaml_path.as_ref();
        debug!("Loading YAML config from: {:?}", yaml_path);

        // Read file content
        let content = tokio::fs::read_to_string(yaml_path)
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ConfigSyncError::YamlReadError(format!("File not found: {:?}", yaml_path))
                } else {
                    ConfigSyncError::IoError(e)
                }
            })?;

        // Parse YAML
        let yaml_config: StreamConfigYaml = serde_yaml::from_str(&content)?;

        // Convert to StreamConfig
        let config = yaml_config.to_stream_config()?;

        // Validate
        config.validate()?;

        debug!("Successfully loaded config for stream: {}", config.stream_id);
        Ok(config)
    }

    /// Discover all stream config YAML files in the config directory
    pub async fn discover_stream_configs(&self) -> Result<Vec<PathBuf>, ConfigSyncError> {
        debug!("Discovering stream configs in: {:?}", self.config_dir);

        // Check if directory exists
        if !tokio::fs::try_exists(&self.config_dir).await? {
            return Err(ConfigSyncError::DirectoryNotFound(
                format!("Directory not found: {:?}", self.config_dir)
            ));
        }

        let mut configs = Vec::new();

        // Walk through subdirectories recursively
        self.discover_configs_recursive(&self.config_dir, &mut configs).await?;

        info!("Discovered {} config files", configs.len());
        Ok(configs)
    }

    /// Recursively discover config.yaml files
    fn discover_configs_recursive<'a>(&'a self, dir: &'a Path, configs: &'a mut Vec<PathBuf>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), ConfigSyncError>> + 'a>> {
        Box::pin(async move {
            let mut read_dir = tokio::fs::read_dir(dir).await?;

            while let Some(entry) = read_dir.next_entry().await? {
                let path = entry.path();
                let metadata = tokio::fs::metadata(&path).await?;

                if metadata.is_dir() {
                    // Check for config.yaml in this directory
                    let config_path = path.join("config.yaml");
                    if tokio::fs::try_exists(&config_path).await? {
                        debug!("Found config: {:?}", config_path);
                        configs.push(config_path);
                    }

                    // Recurse into subdirectory
                    self.discover_configs_recursive(&path, configs).await?;
                }
            }

            Ok(())
        })
    }

    /// Save a stream config to the registry
    pub async fn save_to_registry(
        &self,
        registry: &StreamRegistry,
        config: &StreamConfig,
    ) -> Result<(), ConfigSyncError> {
        debug!("Saving config to registry: {}", config.stream_id);

        // Validate before saving
        config.validate()?;

        // Save to registry
        registry.save_stream(config).await?;

        info!("Saved stream config to registry: {}", config.stream_id);
        Ok(())
    }

    /// Sync all configs from directory to registry
    pub async fn sync_all(&self, registry: &StreamRegistry) -> Result<usize, ConfigSyncError> {
        info!("Starting sync_all operation");

        let config_paths = self.discover_stream_configs().await?;
        let mut synced_count = 0;

        for path in config_paths {
            match self.load_yaml_config(&path).await {
                Ok(config) => {
                    if !config.enabled {
                        warn!("Skipping disabled stream: {}", config.stream_id);
                        continue;
                    }

                    match self.save_to_registry(registry, &config).await {
                        Ok(_) => {
                            synced_count += 1;
                        }
                        Err(e) => {
                            error!("Failed to save stream {}: {}", config.stream_id, e);
                            // Continue with other configs
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to load config from {:?}: {}", path, e);
                    // Continue with other configs
                }
            }
        }

        info!("Sync complete: {} configs synced", synced_count);
        Ok(synced_count)
    }
}

/// YAML representation of stream config (matches file format)
#[derive(Debug, Clone, Deserialize)]
struct StreamConfigYaml {
    stream_id: String,
    description: String,
    #[serde(default = "default_version")]
    version: String,
    #[serde(default = "default_enabled")]
    enabled: bool,
    #[serde(default)]
    retention_days: u32,
    #[serde(default)]
    compression_after_days: u32,
    #[serde(default = "default_partitioning")]
    partitioning_strategy: String,
    /// Fields can be either a HashMap (legacy) or Vec (new format)
    #[serde(default)]
    fields: FieldsYaml,
    /// Sources array (new format)
    #[serde(default)]
    sources: Vec<SourceYaml>,
    #[serde(flatten)]
    extra: std::collections::HashMap<String, serde_yaml::Value>,
}

/// Fields can be either a HashMap or Vec
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(untagged)]
enum FieldsYaml {
    #[default]
    Empty,
    Map(std::collections::HashMap<String, FieldYaml>),
    Array(Vec<FieldYamlWithName>),
}

/// Field YAML with name included (for array format)
#[derive(Debug, Clone, Deserialize)]
struct FieldYamlWithName {
    name: String,
    #[serde(rename = "type")]
    field_type: String,
    #[serde(default = "default_nullable")]
    nullable: bool,
    unit: Option<String>,
    description: Option<String>,
    range: Option<Vec<f64>>,
    display_precision: Option<u32>,
}

/// Source YAML for sources array
#[derive(Debug, Clone, Deserialize)]
struct SourceYaml {
    #[serde(rename = "type")]
    source_type: String,
    #[serde(default = "default_enabled")]
    enabled: bool,
    #[serde(flatten)]
    params: std::collections::HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct FieldYaml {
    #[serde(rename = "type")]
    field_type: String,
    #[serde(default = "default_nullable")]
    nullable: bool,
    unit: Option<String>,
    description: Option<String>,
    range: Option<Vec<f64>>,
    display_precision: Option<u32>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

fn default_enabled() -> bool {
    true
}

fn default_nullable() -> bool {
    true
}

fn default_partitioning() -> String {
    "daily".to_string()
}

impl StreamConfigYaml {
    /// Convert YAML structure to StreamConfig
    fn to_stream_config(&self) -> Result<StreamConfig, ConfigSyncError> {
        use neural_core::{FieldType, SchemaField, SourceConfig, SourceType, StorageConfig};

        // Convert fields - handle both map and array formats
        let mut fields = Vec::new();
        match &self.fields {
            FieldsYaml::Map(map) => {
                for (name, field_yaml) in map {
                    let field_type = parse_field_type(&field_yaml.field_type)?;
                    let mut field = SchemaField::new(name.clone(), field_type);
                    field.nullable = field_yaml.nullable;
                    field.unit = field_yaml.unit.clone();
                    field.description = field_yaml.description.clone();
                    field.range = field_yaml.range.clone();
                    field.display_precision = field_yaml.display_precision;
                    fields.push(field);
                }
            }
            FieldsYaml::Array(arr) => {
                for field_yaml in arr {
                    let field_type = parse_field_type(&field_yaml.field_type)?;
                    let mut field = SchemaField::new(field_yaml.name.clone(), field_type);
                    field.nullable = field_yaml.nullable;
                    field.unit = field_yaml.unit.clone();
                    field.description = field_yaml.description.clone();
                    field.range = field_yaml.range.clone();
                    field.display_precision = field_yaml.display_precision;
                    fields.push(field);
                }
            }
            FieldsYaml::Empty => {}
        }

        // Convert sources - handle both explicit sources array and legacy top-level keys
        let mut sources = Vec::new();

        // First, check explicit sources array (new format)
        for source_yaml in &self.sources {
            let source_type = parse_source_type(&source_yaml.source_type)?;

            // Convert YAML params to JSON params, filtering out 'enabled' to avoid duplicate field
            let params: std::collections::HashMap<String, serde_json::Value> = source_yaml.params
                .iter()
                .filter(|(k, _)| k.as_str() != "enabled") // Filter out enabled - handled by explicit field
                .filter_map(|(k, v)| {
                    yaml_to_json(v).ok().map(|json_v| (k.clone(), json_v))
                })
                .collect();

            sources.push(SourceConfig {
                source_type,
                enabled: source_yaml.enabled,
                params,
            });
        }

        // If no explicit sources, check legacy format (top-level mqtt/http_poll keys)
        if sources.is_empty() {
            for (key, value) in &self.extra {
                let source_type_opt = match key.as_str() {
                    "mqtt" => Some(SourceType::Mqtt),
                    "http_poll" => Some(SourceType::HttpPoll),
                    "webhook" => Some(SourceType::Webhook),
                    "file_watch" => Some(SourceType::FileWatch),
                    _ => None,
                };

                if let Some(source_type) = source_type_opt {
                    if let serde_yaml::Value::Mapping(map) = value {
                        let enabled = map
                            .get(&serde_yaml::Value::String("enabled".to_string()))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true);

                        // Convert YAML mapping to JSON params, filtering out 'enabled'
                        let params: std::collections::HashMap<String, serde_json::Value> = map
                            .iter()
                            .filter_map(|(k, v)| {
                                if let serde_yaml::Value::String(key) = k {
                                    // Filter out enabled - handled by explicit field
                                    if key == "enabled" {
                                        return None;
                                    }
                                    yaml_to_json(v).ok().map(|json_v| (key.clone(), json_v))
                                } else {
                                    None
                                }
                            })
                            .collect();

                        sources.push(SourceConfig {
                            source_type,
                            enabled,
                            params,
                        });
                    }
                }
            }
        }

        // Extract storage config
        let storage = if let Some(storage_yaml) = self.extra.get("storage") {
            if let serde_yaml::Value::Mapping(map) = storage_yaml {
                let batch_size = map
                    .get(&serde_yaml::Value::String("batch_size".to_string()))
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(100);

                let batch_timeout_secs = map
                    .get(&serde_yaml::Value::String("batch_timeout_secs".to_string()))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(5);

                let buffer_capacity = map
                    .get(&serde_yaml::Value::String("buffer_capacity".to_string()))
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(1000);

                Some(StorageConfig {
                    batch_size,
                    batch_timeout_secs,
                    buffer_capacity,
                })
            } else {
                None
            }
        } else {
            None
        };

        Ok(StreamConfig {
            stream_id: self.stream_id.clone(),
            description: self.description.clone(),
            version: self.version.clone(),
            enabled: self.enabled,
            retention_days: self.retention_days,
            compression_after_days: self.compression_after_days,
            partitioning_strategy: self.partitioning_strategy.clone(),
            fields,
            sources,
            storage,
        })
    }
}

/// Parse field type string to FieldType enum
fn parse_field_type(s: &str) -> Result<neural_core::FieldType, ConfigSyncError> {
    use neural_core::FieldType;
    match s.to_lowercase().as_str() {
        "float" => Ok(FieldType::Float),
        "int" => Ok(FieldType::Int),
        "string" => Ok(FieldType::String),
        "bool" => Ok(FieldType::Bool),
        "json" => Ok(FieldType::Json),
        other => Err(ConfigSyncError::InvalidConfig(format!(
            "Unknown field type: {}",
            other
        ))),
    }
}

/// Parse source type string to SourceType enum
fn parse_source_type(s: &str) -> Result<neural_core::SourceType, ConfigSyncError> {
    use neural_core::SourceType;
    match s.to_lowercase().as_str() {
        "mqtt" => Ok(SourceType::Mqtt),
        "http_poll" | "httppoll" => Ok(SourceType::HttpPoll),
        "webhook" => Ok(SourceType::Webhook),
        "file_watch" | "filewatch" => Ok(SourceType::FileWatch),
        other => Err(ConfigSyncError::InvalidConfig(format!(
            "Unknown source type: {}",
            other
        ))),
    }
}

/// Convert YAML Value to JSON Value
fn yaml_to_json(yaml: &serde_yaml::Value) -> Result<serde_json::Value, ConfigSyncError> {
    use serde_yaml::Value as Y;
    use serde_json::Value as J;

    let json = match yaml {
        Y::Null => J::Null,
        Y::Bool(b) => J::Bool(*b),
        Y::Number(n) => {
            if let Some(i) = n.as_i64() {
                J::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                serde_json::Number::from_f64(f)
                    .map(J::Number)
                    .unwrap_or(J::Null)
            } else {
                J::Null
            }
        }
        Y::String(s) => J::String(s.clone()),
        Y::Sequence(seq) => {
            let arr: Result<Vec<_>, _> = seq.iter().map(yaml_to_json).collect();
            J::Array(arr?)
        }
        Y::Mapping(map) => {
            let mut obj = serde_json::Map::new();
            for (k, v) in map {
                if let Y::String(key) = k {
                    obj.insert(key.clone(), yaml_to_json(v)?);
                }
            }
            J::Object(obj)
        }
        Y::Tagged(tagged) => yaml_to_json(&tagged.value)?,
    };

    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use neural_core::{FieldType, SchemaField, SourceType};

    // ========== LONDON SCHOOL TDD: CONFIG SYNC SERVICE TESTS ==========
    // Focus: Behavior verification and interaction testing with mocks

    /// Test that load_yaml_config correctly parses the outdoor-weather config file
    /// Verifies all fields are parsed including stream metadata, fields array, and sources
    #[tokio::test]
    async fn test_load_yaml_config_parses_outdoor_weather_config() {
        // Arrange: Create service pointing to actual config directory
        // Use absolute path from workspace root
        let workspace_root = std::env::var("CARGO_MANIFEST_DIR")
            .map(|p| std::path::PathBuf::from(p).parent().unwrap().parent().unwrap().to_path_buf())
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let config_dir = workspace_root.join("config/base/streams");
        let config_path = config_dir.join("outdoor-weather/config.yaml");

        let service = ConfigSyncService::new(&config_dir);

        // Act: Load the actual YAML config
        let result = service.load_yaml_config(config_path).await;

        // Assert: Verify config was parsed successfully
        assert!(result.is_ok(), "Failed to parse outdoor-weather config: {:?}", result.err());

        let config = result.unwrap();

        // Verify stream metadata
        assert_eq!(config.stream_id, "outdoor-weather");
        assert_eq!(config.description, "Outdoor weather data from OpenWeatherMap Current Weather API");
        assert_eq!(config.version, "1.0.0");
        assert!(config.enabled);
        assert_eq!(config.retention_days, 90);
        assert_eq!(config.compression_after_days, 7);
        assert_eq!(config.partitioning_strategy, "daily");

        // Verify fields were parsed (should have 11 fields)
        assert_eq!(config.fields.len(), 11, "Expected 11 weather fields");

        // Verify specific field details
        let temp_field = config.fields.iter().find(|f| f.name == "temperature");
        assert!(temp_field.is_some(), "temperature field should exist");
        let temp_field = temp_field.unwrap();
        assert_eq!(temp_field.field_type, FieldType::Float);
        assert!(!temp_field.nullable);
        assert_eq!(temp_field.unit, Some("celsius".to_string()));
        assert_eq!(temp_field.range, Some(vec![-50.0, 60.0]));

        // Verify humidity field
        let humidity_field = config.fields.iter().find(|f| f.name == "humidity");
        assert!(humidity_field.is_some(), "humidity field should exist");
        let humidity_field = humidity_field.unwrap();
        assert_eq!(humidity_field.field_type, FieldType::Float);
        assert!(humidity_field.nullable);
        assert_eq!(humidity_field.unit, Some("percent".to_string()));

        // Verify storage config
        assert!(config.storage.is_some());
        let storage = config.storage.unwrap();
        assert_eq!(storage.batch_size, 50);
        assert_eq!(storage.batch_timeout_secs, 30);
        assert_eq!(storage.buffer_capacity, 500);
    }

    /// Test that load_yaml_config correctly parses sources array with http_poll type
    /// Verifies the new sources array format with nested endpoint configurations
    #[tokio::test]
    async fn test_load_yaml_config_parses_sources_array() {
        // Arrange: Create service and path to air quality config
        let workspace_root = std::env::var("CARGO_MANIFEST_DIR")
            .map(|p| std::path::PathBuf::from(p).parent().unwrap().parent().unwrap().to_path_buf())
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let config_dir = workspace_root.join("config/base/streams");
        let config_path = config_dir.join("outdoor-air-quality/config.yaml");

        let service = ConfigSyncService::new(&config_dir);

        // Act: Load config with sources array
        let result = service.load_yaml_config(config_path).await;

        // Assert: Verify sources were parsed
        assert!(result.is_ok(), "Failed to parse air-quality config: {:?}", result.err());

        let config = result.unwrap();

        // Verify stream has sources
        assert_eq!(config.sources.len(), 1, "Expected 1 source");

        let source = &config.sources[0];
        assert_eq!(source.source_type, SourceType::HttpPoll);
        assert!(source.enabled);

        // Verify source params contain expected fields
        assert!(source.params.contains_key("poll_interval_secs"));
        assert!(source.params.contains_key("timeout_secs"));
        assert!(source.params.contains_key("parser_name"));
        assert!(source.params.contains_key("endpoints"));

        // Verify poll_interval_secs value
        if let Some(serde_json::Value::Number(interval)) = source.params.get("poll_interval_secs") {
            assert_eq!(interval.as_u64(), Some(600));
        } else {
            panic!("poll_interval_secs should be a number");
        }

        // Verify parser_name value
        if let Some(serde_json::Value::String(parser)) = source.params.get("parser_name") {
            assert_eq!(parser, "openweathermap_air_pollution");
        } else {
            panic!("parser_name should be a string");
        }
    }

    /// Test that endpoints nested structure is properly parsed within sources
    /// Verifies the endpoints array contains location_id, url, and auth configuration
    #[tokio::test]
    async fn test_load_yaml_config_parses_endpoints() {
        // Arrange: Create service
        let workspace_root = std::env::var("CARGO_MANIFEST_DIR")
            .map(|p| std::path::PathBuf::from(p).parent().unwrap().parent().unwrap().to_path_buf())
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let config_dir = workspace_root.join("config/base/streams");
        let config_path = config_dir.join("outdoor-weather/config.yaml");

        let service = ConfigSyncService::new(&config_dir);

        // Act: Load config
        let result = service.load_yaml_config(config_path).await;
        assert!(result.is_ok());

        let config = result.unwrap();
        let source = &config.sources[0];

        // Assert: Verify endpoints array exists and is properly structured
        let endpoints = source.params.get("endpoints");
        assert!(endpoints.is_some(), "endpoints should exist in params");

        if let Some(serde_json::Value::Array(endpoints_array)) = endpoints {
            assert_eq!(endpoints_array.len(), 1, "Expected 1 endpoint");

            let endpoint = &endpoints_array[0];
            assert!(endpoint.is_object(), "Endpoint should be an object");

            if let serde_json::Value::Object(endpoint_obj) = endpoint {
                // Verify endpoint structure
                assert!(endpoint_obj.contains_key("endpoint_id"));
                assert!(endpoint_obj.contains_key("location_id"));
                assert!(endpoint_obj.contains_key("url"));
                assert!(endpoint_obj.contains_key("auth_type"));
                assert!(endpoint_obj.contains_key("auth_key"));
                assert!(endpoint_obj.contains_key("auth_value"));

                // Verify specific values
                if let Some(serde_json::Value::String(endpoint_id)) = endpoint_obj.get("endpoint_id") {
                    assert_eq!(endpoint_id, "openweathermap_weather");
                }

                if let Some(serde_json::Value::String(location_id)) = endpoint_obj.get("location_id") {
                    assert_eq!(location_id, "home");
                }

                if let Some(serde_json::Value::String(auth_type)) = endpoint_obj.get("auth_type") {
                    assert_eq!(auth_type, "query_param");
                }
            } else {
                panic!("Endpoint should be an object");
            }
        } else {
            panic!("endpoints should be an array");
        }
    }

    /// Test that discover_stream_configs finds all YAML files in subdirectories
    /// Verifies recursive directory traversal and config.yaml discovery
    #[tokio::test]
    async fn test_discover_stream_configs_finds_all_yaml_files() {
        // Arrange: Create service pointing to config directory
        let workspace_root = std::env::var("CARGO_MANIFEST_DIR")
            .map(|p| std::path::PathBuf::from(p).parent().unwrap().parent().unwrap().to_path_buf())
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let config_dir = workspace_root.join("config/base/streams");

        let service = ConfigSyncService::new(&config_dir);

        // Act: Discover all config files
        let result = service.discover_stream_configs().await;

        // Assert: Verify discovery succeeded
        assert!(result.is_ok(), "Failed to discover configs: {:?}", result.err());

        let configs = result.unwrap();

        // Should find at least outdoor-weather and outdoor-air-quality
        assert!(configs.len() >= 2, "Expected at least 2 config files, found {}", configs.len());

        // Verify both expected configs are found
        let has_weather = configs.iter().any(|p| {
            p.to_string_lossy().contains("outdoor-weather") && p.ends_with("config.yaml")
        });
        let has_air_quality = configs.iter().any(|p| {
            p.to_string_lossy().contains("outdoor-air-quality") && p.ends_with("config.yaml")
        });

        assert!(has_weather, "Should find outdoor-weather/config.yaml");
        assert!(has_air_quality, "Should find outdoor-air-quality/config.yaml");

        // Verify all discovered paths end with config.yaml
        for path in &configs {
            assert!(path.ends_with("config.yaml"), "All discovered paths should end with config.yaml: {:?}", path);
        }
    }

    /// Test that discover_stream_configs returns error for non-existent directory
    /// Verifies proper error handling for invalid paths
    #[tokio::test]
    async fn test_discover_stream_configs_nonexistent_directory() {
        // Arrange: Create service with invalid directory
        let service = ConfigSyncService::new("/nonexistent/directory");

        // Act: Attempt to discover configs
        let result = service.discover_stream_configs().await;

        // Assert: Verify appropriate error is returned
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigSyncError::DirectoryNotFound(_)));
    }

    /// Test that SyncReport correctly tracks success, failures, and skips
    /// Verifies the reporting mechanism for sync operations
    #[test]
    fn test_sync_report_tracks_success_and_failures() {
        // Arrange: Create empty report
        let mut report = SyncReport::new();

        // Act: Add various outcomes
        report.synced.push("stream-1".to_string());
        report.synced.push("stream-2".to_string());
        report.failed.push(("stream-3".to_string(), "Parse error".to_string()));
        report.skipped.push("stream-4".to_string());

        // Assert: Verify tracking methods
        assert_eq!(report.synced.len(), 2);
        assert_eq!(report.failed.len(), 1);
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.total(), 4);
        assert!(report.has_failures());
        assert!(!report.is_success());

        // Test success case
        let mut success_report = SyncReport::new();
        success_report.synced.push("stream-1".to_string());
        success_report.skipped.push("stream-2".to_string());

        assert!(!success_report.has_failures());
        assert!(success_report.is_success());
    }

    /// Test that SyncReport default constructor creates empty report
    #[test]
    fn test_sync_report_default() {
        // Arrange & Act: Use default constructor
        let report = SyncReport::default();

        // Assert: Verify empty state
        assert_eq!(report.synced.len(), 0);
        assert_eq!(report.failed.len(), 0);
        assert_eq!(report.skipped.len(), 0);
        assert_eq!(report.total(), 0);
        assert!(!report.has_failures());
        assert!(report.is_success());
    }

    /// Test that load_yaml_config fails gracefully for non-existent file
    /// Verifies error handling for file not found scenarios
    #[tokio::test]
    async fn test_load_yaml_config_file_not_found() {
        // Arrange: Create service
        let workspace_root = std::env::var("CARGO_MANIFEST_DIR")
            .map(|p| std::path::PathBuf::from(p).parent().unwrap().parent().unwrap().to_path_buf())
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let config_dir = workspace_root.join("config/base/streams");
        let nonexistent_path = config_dir.join("nonexistent/config.yaml");

        let service = ConfigSyncService::new(&config_dir);

        // Act: Attempt to load non-existent file
        let result = service.load_yaml_config(nonexistent_path).await;

        // Assert: Verify appropriate error
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigSyncError::YamlReadError(_)));
    }

    /// Test that load_yaml_config validates the config after parsing
    /// Verifies that invalid configs are rejected
    #[tokio::test]
    async fn test_load_yaml_config_validates_config() {
        // Arrange: Create a temporary invalid YAML file
        use std::io::Write;
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("invalid_config.yaml");

        let invalid_yaml = r#"
stream_id: test-stream
description: Test stream
version: "1.0.0"
enabled: true
fields: []
sources: []
"#;

        let mut file = std::fs::File::create(&temp_file).unwrap();
        file.write_all(invalid_yaml.as_bytes()).unwrap();

        let service = ConfigSyncService::new(&temp_dir);

        // Act: Attempt to load invalid config (empty fields and sources)
        let result = service.load_yaml_config(&temp_file).await;

        // Assert: Verify validation error
        assert!(result.is_err(), "Should reject config with no fields or sources");

        // Cleanup
        std::fs::remove_file(temp_file).ok();
    }

    /// Test that ConfigSyncService new() accepts various path types
    /// Verifies flexible path handling
    #[test]
    fn test_config_sync_service_new_accepts_paths() {
        // Arrange & Act: Create service with different path types
        let _service1 = ConfigSyncService::new("config/base/streams");
        let _service2 = ConfigSyncService::new(PathBuf::from("config/base/streams"));
        let path = Path::new("config/base/streams");
        let _service3 = ConfigSyncService::new(path);

        // Assert: All constructions should succeed (compile-time check mostly)
        assert!(true, "All path types should be accepted");
    }

    /// Test that field types are correctly parsed from YAML strings
    /// Verifies parse_field_type function behavior
    #[test]
    fn test_parse_field_type_all_types() {
        // Arrange & Act & Assert: Test all valid field types
        assert_eq!(parse_field_type("float").unwrap(), FieldType::Float);
        assert_eq!(parse_field_type("int").unwrap(), FieldType::Int);
        assert_eq!(parse_field_type("string").unwrap(), FieldType::String);
        assert_eq!(parse_field_type("bool").unwrap(), FieldType::Bool);
        assert_eq!(parse_field_type("json").unwrap(), FieldType::Json);

        // Test case-insensitivity
        assert_eq!(parse_field_type("FLOAT").unwrap(), FieldType::Float);
        assert_eq!(parse_field_type("Float").unwrap(), FieldType::Float);

        // Test invalid type
        let result = parse_field_type("invalid_type");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigSyncError::InvalidConfig(_)));
    }

    /// Test that source types are correctly parsed from YAML strings
    /// Verifies parse_source_type function behavior
    #[test]
    fn test_parse_source_type_all_types() {
        // Arrange & Act & Assert: Test all valid source types
        assert_eq!(parse_source_type("mqtt").unwrap(), SourceType::Mqtt);
        assert_eq!(parse_source_type("http_poll").unwrap(), SourceType::HttpPoll);
        assert_eq!(parse_source_type("httppoll").unwrap(), SourceType::HttpPoll);
        assert_eq!(parse_source_type("webhook").unwrap(), SourceType::Webhook);
        assert_eq!(parse_source_type("file_watch").unwrap(), SourceType::FileWatch);
        assert_eq!(parse_source_type("filewatch").unwrap(), SourceType::FileWatch);

        // Test case-insensitivity
        assert_eq!(parse_source_type("HTTP_POLL").unwrap(), SourceType::HttpPoll);

        // Test invalid type
        let result = parse_source_type("invalid_source");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigSyncError::InvalidConfig(_)));
    }

    /// Test that outdoor-air-quality config parses correctly with all fields
    /// Verifies complete parsing of air quality stream configuration
    #[tokio::test]
    async fn test_load_yaml_config_parses_air_quality_fields() {
        // Arrange: Create service
        let workspace_root = std::env::var("CARGO_MANIFEST_DIR")
            .map(|p| std::path::PathBuf::from(p).parent().unwrap().parent().unwrap().to_path_buf())
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let config_dir = workspace_root.join("config/base/streams");
        let config_path = config_dir.join("outdoor-air-quality/config.yaml");

        let service = ConfigSyncService::new(&config_dir);

        // Act: Load config
        let result = service.load_yaml_config(config_path).await;

        // Assert: Verify parsing succeeded
        assert!(result.is_ok(), "Failed to parse air-quality config: {:?}", result.err());

        let config = result.unwrap();

        // Verify stream metadata
        assert_eq!(config.stream_id, "outdoor-air-quality");
        assert_eq!(config.description, "Outdoor air quality data from OpenWeatherMap Air Pollution API");

        // Verify fields count (should have 9 pollutant fields)
        assert_eq!(config.fields.len(), 9, "Expected 9 air quality fields");

        // Verify AQI field (required)
        let aqi_field = config.fields.iter().find(|f| f.name == "aqi");
        assert!(aqi_field.is_some(), "aqi field should exist");
        let aqi_field = aqi_field.unwrap();
        assert_eq!(aqi_field.field_type, FieldType::Float);
        assert!(!aqi_field.nullable);
        assert_eq!(aqi_field.range, Some(vec![1.0, 5.0]));

        // Verify PM2.5 field (required)
        let pm25_field = config.fields.iter().find(|f| f.name == "pm2_5");
        assert!(pm25_field.is_some(), "pm2_5 field should exist");
        let pm25_field = pm25_field.unwrap();
        assert_eq!(pm25_field.field_type, FieldType::Float);
        assert!(!pm25_field.nullable);

        // Verify optional pollutant fields
        let co_field = config.fields.iter().find(|f| f.name == "co");
        assert!(co_field.is_some(), "co field should exist");
        assert!(co_field.unwrap().nullable);
    }

    // ========== ERROR CONVERSION TESTS ==========

    /// Test that ConfigSyncError properly converts from various error types
    #[test]
    fn test_config_sync_error_conversions() {
        // Test serde_yaml::Error conversion
        let yaml_error = serde_yaml::from_str::<StreamConfigYaml>("invalid: yaml: [").unwrap_err();
        let sync_error: ConfigSyncError = yaml_error.into();
        assert!(matches!(sync_error, ConfigSyncError::YamlParseError(_)));

        // Test StreamConfigError conversion
        let config_error = neural_core::StreamConfigError::NoFields;
        let sync_error: ConfigSyncError = config_error.into();
        assert!(matches!(sync_error, ConfigSyncError::InvalidConfig(_)));
    }
}
