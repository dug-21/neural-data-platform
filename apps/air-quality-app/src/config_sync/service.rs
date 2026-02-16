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
    #[error("Failed to read config file: {0}")]
    ConfigReadError(String),

    #[error("Failed to read YAML file: {0}")]
    YamlReadError(String),

    #[error("Failed to parse YAML: {0}")]
    YamlParseError(String),

    #[error("Failed to parse JSON: {0}")]
    JsonParseError(String),

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

impl From<serde_json::Error> for ConfigSyncError {
    fn from(e: serde_json::Error) -> Self {
        ConfigSyncError::JsonParseError(e.to_string())
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
        info!(
            "Initializing ConfigSyncService with config_dir: {:?}",
            config_dir.as_ref()
        );
        Self {
            config_dir: config_dir.as_ref().to_path_buf(),
        }
    }

    /// Load a single stream config from a YAML file
    pub async fn load_yaml_config(
        &self,
        yaml_path: impl AsRef<Path>,
    ) -> Result<StreamConfig, ConfigSyncError> {
        let yaml_path = yaml_path.as_ref();
        debug!("Loading YAML config from: {:?}", yaml_path);

        // Read file content
        let content = tokio::fs::read_to_string(yaml_path).await.map_err(|e| {
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

        debug!(
            "Successfully loaded config for stream: {}",
            config.stream_id
        );
        Ok(config)
    }

    /// Load a stream config from a JSON file (DP-018: pass-through architecture)
    ///
    /// This method deserializes JSON directly to StreamConfig without any
    /// intermediate transformation, ensuring all fields (including silver_etl)
    /// are preserved exactly as specified in the config file.
    pub async fn load_json_config(
        &self,
        json_path: impl AsRef<Path>,
    ) -> Result<StreamConfig, ConfigSyncError> {
        let json_path = json_path.as_ref();
        debug!("Loading JSON config from: {:?}", json_path);

        // Read file content
        let content = tokio::fs::read_to_string(json_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ConfigSyncError::ConfigReadError(format!("File not found: {:?}", json_path))
            } else {
                ConfigSyncError::IoError(e)
            }
        })?;

        // Parse JSON directly to StreamConfig (pass-through, no transformation)
        let config: StreamConfig = serde_json::from_str(&content)?;

        // Validate
        config.validate()?;

        info!(
            "config loaded from JSON: {} (silver_etl: {})",
            config.stream_id,
            config.silver_etl.is_some()
        );
        Ok(config)
    }

    /// Load a stream config, auto-detecting format based on file extension
    ///
    /// Supports both .json (preferred) and .yaml files.
    /// JSON files are loaded via pass-through (no transformation).
    /// YAML files are loaded via the legacy transformation path.
    pub async fn load_config(
        &self,
        config_path: impl AsRef<Path>,
    ) -> Result<StreamConfig, ConfigSyncError> {
        let config_path = config_path.as_ref();

        if config_path.extension().map_or(false, |ext| ext == "json") {
            self.load_json_config(config_path).await
        } else {
            // Fall back to YAML loading for .yaml/.yml files
            self.load_yaml_config(config_path).await
        }
    }

    /// Discover all stream config files in the config directory
    ///
    /// DP-018: Prefers config.json over config.yaml when both exist.
    /// This supports gradual migration from YAML to JSON format.
    pub async fn discover_stream_configs(&self) -> Result<Vec<PathBuf>, ConfigSyncError> {
        debug!("Discovering stream configs in: {:?}", self.config_dir);

        // Check if directory exists
        if !tokio::fs::try_exists(&self.config_dir).await? {
            return Err(ConfigSyncError::DirectoryNotFound(format!(
                "Directory not found: {:?}",
                self.config_dir
            )));
        }

        let mut configs = Vec::new();

        // Walk through subdirectories recursively
        self.discover_configs_recursive(&self.config_dir, &mut configs)
            .await?;

        info!("Discovered {} config files", configs.len());
        Ok(configs)
    }

    /// Recursively discover config files (DP-018: prefers JSON over YAML)
    fn discover_configs_recursive<'a>(
        &'a self,
        dir: &'a Path,
        configs: &'a mut Vec<PathBuf>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), ConfigSyncError>> + 'a>>
    {
        Box::pin(async move {
            let mut read_dir = tokio::fs::read_dir(dir).await?;

            while let Some(entry) = read_dir.next_entry().await? {
                let path = entry.path();
                let metadata = tokio::fs::metadata(&path).await?;

                if metadata.is_dir() {
                    // DP-018: Prefer config.json over config.yaml
                    let json_path = path.join("config.json");
                    let yaml_path = path.join("config.yaml");

                    if tokio::fs::try_exists(&json_path).await? {
                        // JSON takes priority (pass-through architecture)
                        debug!("Found JSON config: {:?}", json_path);
                        configs.push(json_path);
                    } else if tokio::fs::try_exists(&yaml_path).await? {
                        // Fall back to YAML for backward compatibility
                        debug!("Found YAML config (legacy): {:?}", yaml_path);
                        configs.push(yaml_path);
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
    ///
    /// DP-018 Task 1.7: Sync failures are logged as ERROR level (not WARN).
    /// Returns SyncReport with detailed success/failure information.
    /// Uses auto-detection to load both JSON and YAML configs.
    /// JSON files are loaded via pass-through (preserving silver_etl).
    /// YAML files use the legacy transformation path.
    pub async fn sync_all(&self, registry: &StreamRegistry) -> Result<SyncReport, ConfigSyncError> {
        info!("[sync] Starting sync_all operation");

        let config_paths = self.discover_stream_configs().await?;
        let mut report = SyncReport::new();

        for path in config_paths {
            // Extract stream_id from path for error reporting
            let stream_id_from_path = path
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());

            // DP-018: Use load_config which auto-detects format
            match self.load_config(&path).await {
                Ok(config) => {
                    if !config.enabled {
                        debug!("[sync] Skipping disabled stream: {}", config.stream_id);
                        report.skipped.push(config.stream_id.clone());
                        continue;
                    }

                    match self.save_to_registry(registry, &config).await {
                        Ok(_) => {
                            info!(
                                "[sync] config synced to etcd: /streams/{}/config (silver_etl: {})",
                                config.stream_id,
                                config.silver_etl.is_some()
                            );
                            report.synced.push(config.stream_id.clone());
                        }
                        Err(e) => {
                            // dp-018 Task 1.7: ERROR level for sync failures
                            error!("[sync] Failed to save stream {}: {}", config.stream_id, e);
                            report
                                .failed
                                .push((config.stream_id.clone(), e.to_string()));
                        }
                    }
                }
                Err(e) => {
                    // dp-018 Task 1.7: ERROR level for load failures
                    error!("[sync] Failed to load config from {:?}: {}", path, e);
                    report.failed.push((stream_id_from_path, e.to_string()));
                }
            }
        }

        // dp-018 Task 1.7: Summary of failed streams at the end
        if report.has_failures() {
            let failed_ids: Vec<&str> = report.failed.iter().map(|(id, _)| id.as_str()).collect();
            error!(
                "[sync] Sync completed with {} failures: {:?}",
                report.failed.len(),
                failed_ids
            );
        } else {
            info!(
                "[sync] Sync complete: {} synced, {} skipped",
                report.synced.len(),
                report.skipped.len()
            );
        }

        Ok(report)
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
    /// DP-018: Silver ETL configuration (pass-through from YAML)
    #[serde(default)]
    silver_etl: Option<neural_core::config::SilverEtlConfig>,
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
    /// Stable source identifier (AIR-009)
    #[serde(default)]
    ndp_id: Option<String>,
    /// Mutable context attributes (AIR-009)
    #[serde(default)]
    context: Option<serde_yaml::Value>,
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
        use neural_core::{SchemaField, SourceConfig, SourceType, StorageConfig};

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

            // Convert YAML params to JSON params, filtering out fields handled explicitly
            let params: std::collections::HashMap<String, serde_json::Value> = source_yaml
                .params
                .iter()
                .filter(|(k, _)| {
                    // Filter out fields that have dedicated struct fields
                    !matches!(k.as_str(), "enabled" | "ndp_id" | "context")
                })
                .filter_map(|(k, v)| yaml_to_json(v).ok().map(|json_v| (k.clone(), json_v)))
                .collect();

            // Convert context YAML to JSON (AIR-009)
            let context = source_yaml
                .context
                .as_ref()
                .and_then(|v| yaml_to_json(v).ok());

            sources.push(SourceConfig {
                source_type,
                enabled: source_yaml.enabled,
                ndp_id: source_yaml.ndp_id.clone(),
                context,
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

                        // Extract ndp_id from legacy format (AIR-009)
                        let ndp_id = map
                            .get(&serde_yaml::Value::String("ndp_id".to_string()))
                            .and_then(|v| v.as_str())
                            .map(String::from);

                        // Extract context from legacy format (AIR-009)
                        let context = map
                            .get(&serde_yaml::Value::String("context".to_string()))
                            .and_then(|v| yaml_to_json(v).ok());

                        // Convert YAML mapping to JSON params, filtering out explicit fields
                        let params: std::collections::HashMap<String, serde_json::Value> = map
                            .iter()
                            .filter_map(|(k, v)| {
                                if let serde_yaml::Value::String(key) = k {
                                    // Filter out fields handled explicitly
                                    if matches!(key.as_str(), "enabled" | "ndp_id" | "context") {
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
                            ndp_id,
                            context,
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
            stream_type: None,
            description: self.description.clone(),
            version: self.version.clone(),
            enabled: self.enabled,
            retention_days: self.retention_days,
            compression_after_days: self.compression_after_days,
            partitioning_strategy: self.partitioning_strategy.clone(),
            fields,
            sources,
            storage,
            // DP-018: Pass through silver_etl from YAML config
            silver_etl: self.silver_etl.clone(),
            // Entity schemas (v1.0 format, deprecated in v1.1) - None for YAML configs
            // v1.1 configs store metadata directly on SchemaField
            entity_schemas: None,
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
    use serde_json::Value as J;
    use serde_yaml::Value as Y;

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
            .map(|p| {
                std::path::PathBuf::from(p)
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .to_path_buf()
            })
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let config_dir = workspace_root.join("config/base/streams");
        let config_path = config_dir.join("outdoor-weather/config.yaml");

        let service = ConfigSyncService::new(&config_dir);

        // Act: Load the actual YAML config
        let result = service.load_yaml_config(config_path).await;

        // Assert: Verify config was parsed successfully
        assert!(
            result.is_ok(),
            "Failed to parse outdoor-weather config: {:?}",
            result.err()
        );

        let config = result.unwrap();

        // Verify stream metadata
        assert_eq!(config.stream_id, "outdoor-weather");
        assert_eq!(
            config.description,
            "Outdoor weather data from OpenWeatherMap Current Weather API"
        );
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
            .map(|p| {
                std::path::PathBuf::from(p)
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .to_path_buf()
            })
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let config_dir = workspace_root.join("config/base/streams");
        let config_path = config_dir.join("outdoor-air-quality/config.yaml");

        let service = ConfigSyncService::new(&config_dir);

        // Act: Load config with sources array
        let result = service.load_yaml_config(config_path).await;

        // Assert: Verify sources were parsed
        assert!(
            result.is_ok(),
            "Failed to parse air-quality config: {:?}",
            result.err()
        );

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
            .map(|p| {
                std::path::PathBuf::from(p)
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .to_path_buf()
            })
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
                if let Some(serde_json::Value::String(endpoint_id)) =
                    endpoint_obj.get("endpoint_id")
                {
                    assert_eq!(endpoint_id, "openweathermap_weather");
                }

                if let Some(serde_json::Value::String(location_id)) =
                    endpoint_obj.get("location_id")
                {
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
            .map(|p| {
                std::path::PathBuf::from(p)
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .to_path_buf()
            })
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let config_dir = workspace_root.join("config/base/streams");

        let service = ConfigSyncService::new(&config_dir);

        // Act: Discover all config files
        let result = service.discover_stream_configs().await;

        // Assert: Verify discovery succeeded
        assert!(
            result.is_ok(),
            "Failed to discover configs: {:?}",
            result.err()
        );

        let configs = result.unwrap();

        // Should find at least outdoor-weather and outdoor-air-quality
        assert!(
            configs.len() >= 2,
            "Expected at least 2 config files, found {}",
            configs.len()
        );

        // Verify both expected configs are found (DP-018: can be .json or .yaml)
        let has_weather = configs.iter().any(|p| {
            let path_str = p.to_string_lossy();
            path_str.contains("outdoor-weather")
                && (p.ends_with("config.yaml") || p.ends_with("config.json"))
        });
        let has_air_quality = configs.iter().any(|p| {
            let path_str = p.to_string_lossy();
            path_str.contains("outdoor-air-quality")
                && (p.ends_with("config.yaml") || p.ends_with("config.json"))
        });

        assert!(has_weather, "Should find outdoor-weather config");
        assert!(has_air_quality, "Should find outdoor-air-quality config");

        // Verify all discovered paths end with config.yaml or config.json
        for path in &configs {
            assert!(
                path.ends_with("config.yaml") || path.ends_with("config.json"),
                "All discovered paths should end with config.yaml or config.json: {:?}",
                path
            );
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
        assert!(matches!(
            result.unwrap_err(),
            ConfigSyncError::DirectoryNotFound(_)
        ));
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
        report
            .failed
            .push(("stream-3".to_string(), "Parse error".to_string()));
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
            .map(|p| {
                std::path::PathBuf::from(p)
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .to_path_buf()
            })
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let config_dir = workspace_root.join("config/base/streams");
        let nonexistent_path = config_dir.join("nonexistent/config.yaml");

        let service = ConfigSyncService::new(&config_dir);

        // Act: Attempt to load non-existent file
        let result = service.load_yaml_config(nonexistent_path).await;

        // Assert: Verify appropriate error
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConfigSyncError::YamlReadError(_)
        ));
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
        assert!(
            result.is_err(),
            "Should reject config with no fields or sources"
        );

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
        assert!(matches!(
            result.unwrap_err(),
            ConfigSyncError::InvalidConfig(_)
        ));
    }

    /// Test that source types are correctly parsed from YAML strings
    /// Verifies parse_source_type function behavior
    #[test]
    fn test_parse_source_type_all_types() {
        // Arrange & Act & Assert: Test all valid source types
        assert_eq!(parse_source_type("mqtt").unwrap(), SourceType::Mqtt);
        assert_eq!(
            parse_source_type("http_poll").unwrap(),
            SourceType::HttpPoll
        );
        assert_eq!(parse_source_type("httppoll").unwrap(), SourceType::HttpPoll);
        assert_eq!(parse_source_type("webhook").unwrap(), SourceType::Webhook);
        assert_eq!(
            parse_source_type("file_watch").unwrap(),
            SourceType::FileWatch
        );
        assert_eq!(
            parse_source_type("filewatch").unwrap(),
            SourceType::FileWatch
        );

        // Test case-insensitivity
        assert_eq!(
            parse_source_type("HTTP_POLL").unwrap(),
            SourceType::HttpPoll
        );

        // Test invalid type
        let result = parse_source_type("invalid_source");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConfigSyncError::InvalidConfig(_)
        ));
    }

    /// Test that outdoor-air-quality config parses correctly with all fields
    /// Verifies complete parsing of air quality stream configuration
    #[tokio::test]
    async fn test_load_yaml_config_parses_air_quality_fields() {
        // Arrange: Create service
        let workspace_root = std::env::var("CARGO_MANIFEST_DIR")
            .map(|p| {
                std::path::PathBuf::from(p)
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .to_path_buf()
            })
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let config_dir = workspace_root.join("config/base/streams");
        let config_path = config_dir.join("outdoor-air-quality/config.yaml");

        let service = ConfigSyncService::new(&config_dir);

        // Act: Load config
        let result = service.load_yaml_config(config_path).await;

        // Assert: Verify parsing succeeded
        assert!(
            result.is_ok(),
            "Failed to parse air-quality config: {:?}",
            result.err()
        );

        let config = result.unwrap();

        // Verify stream metadata
        assert_eq!(config.stream_id, "outdoor-air-quality");
        assert_eq!(
            config.description,
            "Outdoor air quality data from OpenWeatherMap Air Pollution API"
        );

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

    // ========== AIR-009 TDD CYCLE 10: ndp_id AND context PARSING TESTS ==========

    /// Test that load_yaml_config correctly parses ndp_id from source configuration
    /// Verifies the new AIR-009 stable source identifier field
    #[tokio::test]
    async fn test_load_yaml_config_parses_ndp_id() {
        // Arrange: Create temp YAML with ndp_id
        let yaml_content = r#"
stream_id: "test-stream"
description: "Test stream with ndp_id"
version: "1.0.0"
enabled: true
fields:
  - name: value
    type: float
    nullable: false
sources:
  - type: mqtt
    enabled: true
    ndp_id: "sensor-office-001"
    broker_url: "localhost"
"#;

        // Write to temp file and load
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_ndp_id_config.yaml");
        std::fs::write(&temp_file, yaml_content).unwrap();

        let service = ConfigSyncService::new(&temp_dir);
        let result = service.load_yaml_config(&temp_file).await;

        // Assert
        assert!(result.is_ok(), "Failed to parse config: {:?}", result.err());
        let config = result.unwrap();
        assert_eq!(config.sources.len(), 1);
        assert_eq!(
            config.sources[0].ndp_id,
            Some("sensor-office-001".to_string())
        );

        // Cleanup
        std::fs::remove_file(temp_file).ok();
    }

    /// Test that load_yaml_config correctly parses context from source configuration
    /// Verifies the new AIR-009 mutable context attributes field
    #[tokio::test]
    async fn test_load_yaml_config_parses_context() {
        // Arrange: Create temp YAML with context
        let yaml_content = r#"
stream_id: "test-stream"
description: "Test stream with context"
version: "1.0.0"
enabled: true
fields:
  - name: value
    type: float
    nullable: false
sources:
  - type: mqtt
    enabled: true
    context:
      room: office
      floor: 2
      calibrated: true
    broker_url: "localhost"
"#;

        // Write to temp file and load
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_context_config.yaml");
        std::fs::write(&temp_file, yaml_content).unwrap();

        let service = ConfigSyncService::new(&temp_dir);
        let result = service.load_yaml_config(&temp_file).await;

        // Assert
        assert!(result.is_ok(), "Failed to parse config: {:?}", result.err());
        let config = result.unwrap();
        assert_eq!(config.sources.len(), 1);
        assert!(config.sources[0].context.is_some());
        let ctx = config.sources[0].context.as_ref().unwrap();
        assert_eq!(ctx["room"], "office");
        assert_eq!(ctx["floor"], 2);
        assert_eq!(ctx["calibrated"], true);

        // Cleanup
        std::fs::remove_file(temp_file).ok();
    }

    /// Test that load_yaml_config correctly parses both ndp_id and context together
    /// Verifies that both AIR-009 fields work in combination
    #[tokio::test]
    async fn test_load_yaml_config_parses_ndp_id_and_context_together() {
        // Arrange: Create temp YAML with both ndp_id and context
        let yaml_content = r#"
stream_id: "test-stream"
description: "Test stream with both"
version: "1.0.0"
enabled: true
fields:
  - name: value
    type: float
    nullable: false
sources:
  - type: http_poll
    enabled: true
    ndp_id: "api-weather-home"
    context:
      location: backyard
      sensor_model: BME280
      installation_date: "2024-01-15"
    poll_interval_secs: 300
"#;

        // Write to temp file and load
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_ndp_id_context_config.yaml");
        std::fs::write(&temp_file, yaml_content).unwrap();

        let service = ConfigSyncService::new(&temp_dir);
        let result = service.load_yaml_config(&temp_file).await;

        // Assert
        assert!(result.is_ok(), "Failed to parse config: {:?}", result.err());
        let config = result.unwrap();
        assert_eq!(config.sources.len(), 1);

        let source = &config.sources[0];
        assert_eq!(source.ndp_id, Some("api-weather-home".to_string()));
        assert!(source.context.is_some());

        let ctx = source.context.as_ref().unwrap();
        assert_eq!(ctx["location"], "backyard");
        assert_eq!(ctx["sensor_model"], "BME280");
        assert_eq!(ctx["installation_date"], "2024-01-15");

        // Verify poll_interval_secs is in params (not context)
        assert!(source.params.contains_key("poll_interval_secs"));

        // Cleanup
        std::fs::remove_file(temp_file).ok();
    }

    /// Test that ndp_id and context are NOT in params HashMap (they have dedicated fields)
    /// Verifies proper field separation for AIR-009
    #[tokio::test]
    async fn test_ndp_id_and_context_not_in_params() {
        // Arrange: Create temp YAML with ndp_id and context
        let yaml_content = r#"
stream_id: "test-stream"
description: "Test stream"
version: "1.0.0"
enabled: true
fields:
  - name: value
    type: float
    nullable: false
sources:
  - type: mqtt
    enabled: true
    ndp_id: "sensor-001"
    context:
      room: kitchen
    broker_url: "localhost"
    topic: "sensors/kitchen"
"#;

        // Write to temp file and load
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_params_separation.yaml");
        std::fs::write(&temp_file, yaml_content).unwrap();

        let service = ConfigSyncService::new(&temp_dir);
        let result = service.load_yaml_config(&temp_file).await;

        // Assert
        assert!(result.is_ok());
        let config = result.unwrap();
        let source = &config.sources[0];

        // ndp_id and context should NOT be in params
        assert!(
            !source.params.contains_key("ndp_id"),
            "ndp_id should not be in params"
        );
        assert!(
            !source.params.contains_key("context"),
            "context should not be in params"
        );

        // Other fields should be in params
        assert!(source.params.contains_key("broker_url"));
        assert!(source.params.contains_key("topic"));

        // Cleanup
        std::fs::remove_file(temp_file).ok();
    }

    /// Test that ConfigSyncError properly converts from various error types
    #[test]
    fn test_config_sync_error_conversions() {
        // Test serde_yaml::Error conversion
        let yaml_error = serde_yaml::from_str::<serde_yaml::Value>("invalid: yaml: [").unwrap_err();
        let sync_error: ConfigSyncError = yaml_error.into();
        assert!(matches!(sync_error, ConfigSyncError::YamlParseError(_)));

        // Test StreamConfigError conversion
        let config_error = neural_core::StreamConfigError::NoFields;
        let sync_error: ConfigSyncError = config_error.into();
        assert!(matches!(sync_error, ConfigSyncError::InvalidConfig(_)));
    }

    // ==========================================================================
    // DP-018: JSON PASS-THROUGH TESTS (LONDON TDD)
    // ==========================================================================
    // These tests verify the elimination of the lossy transformation layer.
    // The ConfigSyncService should now:
    // 1. Read JSON files directly (config.json preferred over config.yaml)
    // 2. Deserialize directly to StreamConfig (no intermediate struct)
    // 3. Preserve silver_etl and all other fields without loss

    /// Test that load_json_config reads JSON and deserializes directly to StreamConfig
    /// This is the core pass-through behavior required by ADR-018-001
    #[tokio::test]
    async fn test_load_json_config_deserializes_directly() {
        // Arrange: Create temp JSON config
        let json_content = r#"{
            "stream_id": "test-stream",
            "description": "Test stream for JSON pass-through",
            "version": "1.0.0",
            "enabled": true,
            "retention_days": 365,
            "compression_after_days": 7,
            "partitioning_strategy": "daily",
            "fields": [
                {"name": "pm25", "type": "float", "nullable": false}
            ],
            "sources": [
                {"type": "mqtt", "enabled": true}
            ],
            "storage": {
                "batch_size": 100,
                "batch_timeout_secs": 5,
                "buffer_capacity": 1000
            }
        }"#;

        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_json_passthrough.json");
        std::fs::write(&temp_file, json_content).unwrap();

        let service = ConfigSyncService::new(&temp_dir);

        // Act: Load JSON config
        let result = service.load_json_config(&temp_file).await;

        // Assert
        assert!(
            result.is_ok(),
            "Failed to load JSON config: {:?}",
            result.err()
        );
        let config = result.unwrap();
        assert_eq!(config.stream_id, "test-stream");
        assert_eq!(config.fields.len(), 1);
        assert_eq!(config.sources.len(), 1);

        // Cleanup
        std::fs::remove_file(temp_file).ok();
    }

    /// Test that silver_etl is PRESERVED after loading JSON config
    /// This is the critical test - silver_etl was previously lost in transformation
    #[tokio::test]
    async fn test_load_json_config_preserves_silver_etl() {
        // Arrange: Create JSON config with silver_etl section
        let json_content = r#"{
            "stream_id": "air-quality",
            "description": "Air quality with silver_etl",
            "version": "1.0.0",
            "enabled": true,
            "fields": [
                {"name": "pm25", "type": "float", "nullable": false}
            ],
            "sources": [
                {"type": "mqtt", "enabled": true}
            ],
            "silver_etl": {
                "enabled": true,
                "target_table": "silver.air_quality_observations",
                "timestamp": {
                    "source_field": "timestamp",
                    "target_field": "observation_time",
                    "transform": "microseconds_to_timestamp"
                },
                "field_mappings": [
                    {
                        "source_path": "raw_payload.pm02",
                        "target_column": "pm25",
                        "type": "double_precision",
                        "nullable": false
                    }
                ]
            }
        }"#;

        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_silver_etl_preserved.json");
        std::fs::write(&temp_file, json_content).unwrap();

        let service = ConfigSyncService::new(&temp_dir);

        // Act: Load JSON config
        let result = service.load_json_config(&temp_file).await;

        // Assert: silver_etl MUST be preserved
        assert!(
            result.is_ok(),
            "Failed to load JSON config: {:?}",
            result.err()
        );
        let config = result.unwrap();

        assert!(
            config.silver_etl.is_some(),
            "silver_etl must be preserved after loading!"
        );
        let etl = config.silver_etl.unwrap();
        assert!(etl.enabled, "silver_etl.enabled should be true");
        assert_eq!(etl.target_table, "silver.air_quality_observations");
        assert_eq!(etl.field_mappings.len(), 1);
        assert_eq!(etl.field_mappings[0].target_column, "pm25");

        // Cleanup
        std::fs::remove_file(temp_file).ok();
    }

    /// Test that discover_stream_configs prefers config.json over config.yaml
    #[tokio::test]
    async fn test_discover_prefers_json_over_yaml() {
        // Arrange: Create temp directory with both config.json and config.yaml
        let temp_dir = std::env::temp_dir().join("dp018_prefer_json");
        let stream_dir = temp_dir.join("test-stream");
        std::fs::create_dir_all(&stream_dir).unwrap();

        // Create both files
        let yaml_content = "stream_id: test-stream\ndescription: YAML version";
        let json_content = r#"{"stream_id": "test-stream", "description": "JSON version"}"#;
        std::fs::write(stream_dir.join("config.yaml"), yaml_content).unwrap();
        std::fs::write(stream_dir.join("config.json"), json_content).unwrap();

        let service = ConfigSyncService::new(&temp_dir);

        // Act: Discover configs
        let result = service.discover_stream_configs().await;

        // Assert: Should find config.json (preferred)
        assert!(result.is_ok());
        let configs = result.unwrap();
        assert_eq!(configs.len(), 1);
        assert!(
            configs[0].to_string_lossy().ends_with("config.json"),
            "Should prefer config.json over config.yaml, found: {:?}",
            configs[0]
        );

        // Cleanup
        std::fs::remove_dir_all(temp_dir).ok();
    }

    /// Test that discover_stream_configs falls back to config.yaml when no JSON exists
    #[tokio::test]
    async fn test_discover_falls_back_to_yaml() {
        // Arrange: Create temp directory with only config.yaml
        let temp_dir = std::env::temp_dir().join("dp018_yaml_fallback");
        let stream_dir = temp_dir.join("legacy-stream");
        std::fs::create_dir_all(&stream_dir).unwrap();

        let yaml_content = "stream_id: legacy-stream\ndescription: Legacy YAML";
        std::fs::write(stream_dir.join("config.yaml"), yaml_content).unwrap();

        let service = ConfigSyncService::new(&temp_dir);

        // Act: Discover configs
        let result = service.discover_stream_configs().await;

        // Assert: Should find config.yaml as fallback
        assert!(result.is_ok());
        let configs = result.unwrap();
        assert_eq!(configs.len(), 1);
        assert!(
            configs[0].to_string_lossy().ends_with("config.yaml"),
            "Should fall back to config.yaml"
        );

        // Cleanup
        std::fs::remove_dir_all(temp_dir).ok();
    }

    /// Test that load_config auto-detects format based on file extension
    #[tokio::test]
    async fn test_load_config_auto_detects_format() {
        // Arrange: Create JSON config
        let json_content = r#"{
            "stream_id": "auto-detect",
            "description": "Auto-detect format test",
            "version": "1.0.0",
            "enabled": true,
            "fields": [{"name": "value", "type": "float", "nullable": false}],
            "sources": [{"type": "mqtt", "enabled": true}]
        }"#;

        let temp_dir = std::env::temp_dir();
        let json_file = temp_dir.join("test_auto_detect.json");
        std::fs::write(&json_file, json_content).unwrap();

        let service = ConfigSyncService::new(&temp_dir);

        // Act: Load config (should auto-detect JSON)
        let result = service.load_config(&json_file).await;

        // Assert
        assert!(
            result.is_ok(),
            "Failed to auto-detect JSON format: {:?}",
            result.err()
        );
        let config = result.unwrap();
        assert_eq!(config.stream_id, "auto-detect");

        // Cleanup
        std::fs::remove_file(json_file).ok();
    }

    /// Test that JSON parse errors provide clear messages
    #[tokio::test]
    async fn test_json_parse_error_is_descriptive() {
        // Arrange: Create invalid JSON
        let invalid_json = r#"{ "stream_id": "broken", invalid json here }"#;

        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_invalid_json.json");
        std::fs::write(&temp_file, invalid_json).unwrap();

        let service = ConfigSyncService::new(&temp_dir);

        // Act: Try to load invalid JSON
        let result = service.load_json_config(&temp_file).await;

        // Assert: Should fail with descriptive error
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, ConfigSyncError::JsonParseError(_)));

        // Cleanup
        std::fs::remove_file(temp_file).ok();
    }
}
