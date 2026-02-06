//! Configuration loading for ndp-lib.
//!
//! Provides a trait-based abstraction over configuration sources (files, etcd, mocks)
//! and structs for the parts of stream/dimension configs that sync operations need.

use crate::error::{NdpLibError, Result};
use serde::Deserialize;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// ConfigLoader trait
// ---------------------------------------------------------------------------

/// Trait for loading NDP configurations from any source.
///
/// V1.1.9: `FileSystemConfigLoader` reads from disk.
/// Future: `EtcdConfigLoader`, `MockConfigLoader` for tests.
pub trait ConfigLoader: Send + Sync {
    /// Load all stream configs from the configured source.
    fn load_stream_configs(&self) -> Result<Vec<StreamConfig>>;

    /// Load a dimension config by dimension ID.
    fn load_dimension_config(&self, dimension_id: &str) -> Result<DimensionConfig>;

    /// Load all domain configs. Default returns empty vec (backwards-compatible).
    fn load_domain_configs(&self) -> Result<Vec<DomainConfig>> {
        Ok(vec![])
    }
}

// ---------------------------------------------------------------------------
// StreamConfig -- subset of fields needed by dictionary sync
// ---------------------------------------------------------------------------

/// Stream configuration data needed by dictionary sync.
///
/// Parsed from the top-level keys of `config/base/streams/<id>/config.json`.
/// Uses `serde_json::Value` for sections not yet needed.
#[derive(Debug, Clone, Deserialize)]
pub struct StreamConfig {
    pub stream_id: String,

    #[serde(default)]
    pub description: String,

    #[serde(default)]
    pub version: String,

    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default)]
    pub retention_days: Option<i32>,

    /// Bronze-level field definitions.
    #[serde(default)]
    pub fields: Vec<BronzeField>,

    /// Data sources feeding this stream (MQTT, HTTP, etc.).
    #[serde(default)]
    pub sources: Vec<SourceConfig>,

    /// Silver ETL configuration (optional -- some streams may not have it).
    #[serde(default)]
    pub silver_etl: Option<SilverEtlConfig>,

    /// Entity schemas from the config (optional).
    #[serde(default)]
    pub entity_schemas: Option<serde_json::Value>,
}

fn default_true() -> bool {
    true
}

/// A Bronze-level field definition from the `fields[]` array.
#[derive(Debug, Clone, Deserialize)]
pub struct BronzeField {
    pub name: String,

    #[serde(rename = "type")]
    pub field_type: String,

    #[serde(default)]
    pub nullable: bool,

    #[serde(default)]
    pub unit: Option<String>,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub range: Option<Vec<serde_json::Value>>,
}

/// A data source definition from the `sources[]` array.
///
/// Sources have varied shapes (MQTT vs HTTP), so we capture the common
/// fields and stash everything else into `extra` for the dictionary config blob.
#[derive(Debug, Clone, Deserialize)]
pub struct SourceConfig {
    #[serde(rename = "type")]
    pub source_type: String,

    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default)]
    pub ndp_id: Option<String>,

    #[serde(default)]
    pub parser: Option<serde_json::Value>,

    /// All remaining fields captured as-is for the dictionary config JSONB column.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Silver ETL configuration block.
#[derive(Debug, Clone, Deserialize)]
pub struct SilverEtlConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub target_table: Option<String>,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub grain: Option<String>,

    #[serde(default)]
    pub timestamp: Option<TimestampConfig>,

    #[serde(default)]
    pub field_mappings: Vec<SilverFieldMapping>,

    /// Table-level DQ rules.
    #[serde(default)]
    pub dq_rules: Vec<serde_json::Value>,
}

/// Timestamp mapping within silver_etl.
#[derive(Debug, Clone, Deserialize)]
pub struct TimestampConfig {
    #[serde(default)]
    pub source_field: Option<String>,

    #[serde(default)]
    pub target_field: Option<String>,

    #[serde(default)]
    pub transform: Option<String>,
}

/// A single Silver field mapping from `silver_etl.field_mappings[]`.
#[derive(Debug, Clone, Deserialize)]
pub struct SilverFieldMapping {
    #[serde(default)]
    pub source_path: Option<String>,

    #[serde(default)]
    pub target_column: Option<String>,

    #[serde(rename = "type", default)]
    pub column_type: Option<String>,

    #[serde(default)]
    pub unit: Option<String>,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub nullable: Option<bool>,

    #[serde(default)]
    pub dq_rules: Vec<serde_json::Value>,

    #[serde(default)]
    pub transform: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// DimensionConfig -- parsed from dimension YAML/JSON
// ---------------------------------------------------------------------------

/// Dimension configuration needed by dimension sync.
#[derive(Debug, Clone, Deserialize)]
pub struct DimensionConfig {
    pub dimension_id: String,

    #[serde(default)]
    pub description: String,

    #[serde(default)]
    pub version: String,

    pub target: DimensionTarget,

    pub source: DimensionSource,

    pub schema: DimensionSchema,

    #[serde(default)]
    pub load: Option<DimensionLoad>,
}

/// Target table for a dimension.
#[derive(Debug, Clone, Deserialize)]
pub struct DimensionTarget {
    pub table: String,

    #[serde(default = "default_silver_schema")]
    pub schema: String,
}

fn default_silver_schema() -> String {
    "silver".to_string()
}

/// Source configuration for a dimension.
#[derive(Debug, Clone, Deserialize)]
pub struct DimensionSource {
    #[serde(rename = "type")]
    pub source_type: String,

    #[serde(default)]
    pub path: Option<String>,

    #[serde(default = "default_comma")]
    pub delimiter: String,

    #[serde(default = "default_true")]
    pub has_header: bool,
}

fn default_comma() -> String {
    ",".to_string()
}

/// Schema definition for a dimension.
#[derive(Debug, Clone, Deserialize)]
pub struct DimensionSchema {
    #[serde(default)]
    pub primary_key: Vec<String>,

    #[serde(default)]
    pub fields: Vec<DimensionField>,
}

/// A single field in a dimension schema.
#[derive(Debug, Clone, Deserialize)]
pub struct DimensionField {
    pub name: String,

    #[serde(rename = "type")]
    pub field_type: String,

    #[serde(default)]
    pub nullable: bool,

    #[serde(default)]
    pub description: Option<String>,
}

/// Load strategy for a dimension.
#[derive(Debug, Clone, Deserialize)]
pub struct DimensionLoad {
    #[serde(default = "default_strategy")]
    pub strategy: String,

    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

fn default_strategy() -> String {
    "truncate_and_load".to_string()
}

fn default_batch_size() -> usize {
    1000
}

// ---------------------------------------------------------------------------
// DomainConfig -- parsed from config/domains/*/domain.json
// ---------------------------------------------------------------------------

/// Domain configuration parsed from domain.json.
/// Used by `load_domain_configs()` and converted to `DomainSyncEntry` for sync.
#[derive(Debug, Clone, Deserialize)]
pub struct DomainConfig {
    pub id: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub streams: Vec<DomainStreamConfig>,
    /// Pass-through: consumed by ndp-gold-ddl, not by domain sync.
    #[serde(default)]
    pub alignment: Option<serde_json::Value>,
    /// Pass-through: consumed by ndp-gold-ddl, not by domain sync.
    #[serde(default)]
    pub events: Option<serde_json::Value>,
    #[serde(default)]
    pub objectives: Vec<DomainObjectiveConfig>,
    #[serde(default)]
    pub constraints: Vec<DomainConstraintConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DomainStreamConfig {
    pub stream_id: String,
    pub alias: String,
    #[serde(default = "default_primary")]
    pub role: String,
    #[serde(default)]
    pub null_handling: Option<String>,
}

fn default_primary() -> String {
    "primary".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct DomainObjectiveConfig {
    pub id: String,
    #[serde(default)]
    pub description: Option<String>,
    pub target: ObjectiveTargetConfig,
    #[serde(default = "default_medium")]
    pub priority: String,
}

fn default_medium() -> String {
    "medium".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct ObjectiveTargetConfig {
    pub stream: String,
    pub metric: String,
    pub condition: String,
    pub threshold: f64,
    #[serde(default)]
    pub threshold_upper: Option<f64>,
    #[serde(default)]
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DomainConstraintConfig {
    pub id: String,
    #[serde(default)]
    pub description: Option<String>,
    pub stream: String,
    pub metric: String,
    pub condition: String,
    pub threshold: f64,
    #[serde(default)]
    pub unit: Option<String>,
}

// ---------------------------------------------------------------------------
// FileSystemConfigLoader
// ---------------------------------------------------------------------------

/// Loads stream and dimension configs from the local filesystem.
///
/// Streams are expected at `<streams_dir>/<stream_id>/config.json`.
/// Dimensions are expected at `<dimensions_dir>/<dim_id>.json` (flat file pattern).
pub struct FileSystemConfigLoader {
    streams_dir: PathBuf,
    dimensions_dir: PathBuf,
    domains_dir: Option<PathBuf>,
}

impl FileSystemConfigLoader {
    /// Create a new loader.
    ///
    /// * `streams_dir` - directory containing `<stream_id>/config.json` subdirs
    /// * `dimensions_dir` - directory containing `<dim_id>/config.json` subdirs
    pub fn new(streams_dir: impl Into<PathBuf>, dimensions_dir: impl Into<PathBuf>) -> Self {
        Self {
            streams_dir: streams_dir.into(),
            dimensions_dir: dimensions_dir.into(),
            domains_dir: None,
        }
    }

    /// Create a loader from a base config directory.
    ///
    /// Assumes `<base>/streams/` and `<base>/dimensions/` subdirectories.
    /// The domains directory is NOT set here (it lives at a different path level).
    /// Use `with_domains_dir()` to set it.
    pub fn from_base_dir(base_dir: impl Into<PathBuf>) -> Self {
        let base: PathBuf = base_dir.into();
        Self {
            streams_dir: base.join("streams"),
            dimensions_dir: base.join("dimensions"),
            domains_dir: None,
        }
    }

    /// Set the domains directory for domain config loading.
    pub fn with_domains_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.domains_dir = Some(dir.into());
        self
    }

    /// Discover stream IDs by listing subdirectories of the streams dir.
    fn discover_stream_ids(&self) -> Result<Vec<String>> {
        if !self.streams_dir.exists() {
            return Err(NdpLibError::ConfigNotFound {
                path: self.streams_dir.display().to_string(),
            });
        }

        let mut ids = Vec::new();
        for entry in std::fs::read_dir(&self.streams_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let config_path = path.join("config.json");
                if config_path.exists() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        ids.push(name.to_string());
                    }
                }
            }
        }
        ids.sort();
        Ok(ids)
    }

    /// Load a single stream config by ID.
    fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig> {
        let config_path = self.streams_dir.join(stream_id).join("config.json");
        if !config_path.exists() {
            return Err(NdpLibError::ConfigNotFound {
                path: config_path.display().to_string(),
            });
        }

        let content = std::fs::read_to_string(&config_path)?;
        let config: StreamConfig =
            serde_json::from_str(&content).map_err(|e| NdpLibError::ConfigParse {
                message: format!("Failed to parse {}: {}", config_path.display(), e),
            })?;

        Ok(config)
    }

    /// Discover domain IDs by listing subdirectories of the domains dir.
    fn discover_domain_ids(&self) -> Result<Vec<String>> {
        let domains_dir = match &self.domains_dir {
            Some(d) => d,
            None => {
                return Err(NdpLibError::ConfigNotFound {
                    path: "<domains_dir not configured>".to_string(),
                });
            }
        };

        if !domains_dir.exists() {
            return Err(NdpLibError::ConfigNotFound {
                path: domains_dir.display().to_string(),
            });
        }

        let mut ids = Vec::new();
        for entry in std::fs::read_dir(domains_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let config_path = path.join("domain.json");
                if config_path.exists() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        ids.push(name.to_string());
                    }
                }
            }
        }
        ids.sort();
        Ok(ids)
    }

    /// Load a single domain config by ID.
    fn load_domain_config(&self, domain_id: &str) -> Result<DomainConfig> {
        let domains_dir = match &self.domains_dir {
            Some(d) => d,
            None => {
                return Err(NdpLibError::ConfigNotFound {
                    path: "<domains_dir not configured>".to_string(),
                });
            }
        };

        let config_path = domains_dir.join(domain_id).join("domain.json");
        if !config_path.exists() {
            return Err(NdpLibError::ConfigNotFound {
                path: config_path.display().to_string(),
            });
        }

        let content = std::fs::read_to_string(&config_path)?;
        let config: DomainConfig =
            serde_json::from_str(&content).map_err(|e| NdpLibError::ConfigParse {
                message: format!("Failed to parse {}: {}", config_path.display(), e),
            })?;

        Ok(config)
    }
}

impl ConfigLoader for FileSystemConfigLoader {
    fn load_stream_configs(&self) -> Result<Vec<StreamConfig>> {
        let ids = self.discover_stream_ids()?;
        let mut configs = Vec::with_capacity(ids.len());

        for id in &ids {
            match self.load_stream_config(id) {
                Ok(config) => configs.push(config),
                Err(e) => {
                    tracing::warn!(stream_id = %id, error = %e, "Skipping stream config");
                }
            }
        }

        Ok(configs)
    }

    fn load_dimension_config(&self, dimension_id: &str) -> Result<DimensionConfig> {
        // Flat file pattern: <dimensions_dir>/<dim_id>.json
        let config_path = self.dimensions_dir.join(format!("{}.json", dimension_id));

        if !config_path.exists() {
            return Err(NdpLibError::ConfigNotFound {
                path: config_path.display().to_string(),
            });
        }

        let content = std::fs::read_to_string(&config_path)?;
        let config: DimensionConfig =
            serde_json::from_str(&content).map_err(|e| NdpLibError::ConfigParse {
                message: format!("Failed to parse {}: {}", config_path.display(), e),
            })?;

        Ok(config)
    }

    fn load_domain_configs(&self) -> Result<Vec<DomainConfig>> {
        if self.domains_dir.is_none() {
            return Ok(vec![]);
        }

        let ids = self.discover_domain_ids()?;
        let mut configs = Vec::with_capacity(ids.len());

        for id in &ids {
            match self.load_domain_config(id) {
                Ok(config) => configs.push(config),
                Err(e) => {
                    tracing::warn!(domain_id = %id, error = %e, "Skipping domain config");
                }
            }
        }

        Ok(configs)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::Path;
    use tempfile::TempDir;

    fn write_stream_config(dir: &Path, stream_id: &str, content: &str) {
        let stream_dir = dir.join("streams").join(stream_id);
        std::fs::create_dir_all(&stream_dir).unwrap();
        let mut f = std::fs::File::create(stream_dir.join("config.json")).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_discover_stream_ids() {
        let tmp = TempDir::new().unwrap();
        write_stream_config(tmp.path(), "air-quality", r#"{"stream_id":"air-quality"}"#);
        write_stream_config(
            tmp.path(),
            "outdoor-weather",
            r#"{"stream_id":"outdoor-weather"}"#,
        );

        let loader = FileSystemConfigLoader::from_base_dir(tmp.path());
        let ids = loader.discover_stream_ids().unwrap();
        assert_eq!(ids, vec!["air-quality", "outdoor-weather"]);
    }

    #[test]
    fn test_load_stream_configs() {
        let tmp = TempDir::new().unwrap();
        let config_json = r#"{
            "stream_id": "test-stream",
            "description": "Test stream",
            "version": "1.0.0",
            "enabled": true,
            "retention_days": 90,
            "fields": [
                {"name": "temp", "type": "float", "nullable": false, "unit": "celsius"}
            ],
            "silver_etl": {
                "enabled": true,
                "target_table": "silver.test_table",
                "description": "Test ETL",
                "grain": "1 row per reading",
                "field_mappings": [
                    {
                        "source_path": "raw_payload.temp",
                        "target_column": "temperature_c",
                        "type": "double_precision",
                        "unit": "Celsius",
                        "description": "Temperature",
                        "nullable": false,
                        "dq_rules": []
                    }
                ]
            }
        }"#;

        write_stream_config(tmp.path(), "test-stream", config_json);

        // Also create the dimensions dir so the loader can be constructed
        std::fs::create_dir_all(tmp.path().join("dimensions")).unwrap();

        let loader = FileSystemConfigLoader::from_base_dir(tmp.path());
        let configs = loader.load_stream_configs().unwrap();

        assert_eq!(configs.len(), 1);
        let c = &configs[0];
        assert_eq!(c.stream_id, "test-stream");
        assert_eq!(c.description, "Test stream");
        assert_eq!(c.version, "1.0.0");
        assert!(c.enabled);
        assert_eq!(c.retention_days, Some(90));
        assert_eq!(c.fields.len(), 1);
        assert_eq!(c.fields[0].name, "temp");

        let etl = c.silver_etl.as_ref().unwrap();
        assert!(etl.enabled);
        assert_eq!(etl.target_table.as_deref(), Some("silver.test_table"));
        assert_eq!(etl.field_mappings.len(), 1);
        assert_eq!(
            etl.field_mappings[0].target_column.as_deref(),
            Some("temperature_c")
        );
    }

    #[test]
    fn test_config_not_found() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("streams")).unwrap();
        std::fs::create_dir_all(tmp.path().join("dimensions")).unwrap();

        let loader = FileSystemConfigLoader::from_base_dir(tmp.path());
        let result = loader.load_dimension_config("nonexistent");
        assert!(matches!(result, Err(NdpLibError::ConfigNotFound { .. })));
    }

    #[test]
    fn test_parse_real_outdoor_weather_config() {
        // Verify that our StreamConfig struct can parse the real outdoor-weather config.
        let content = include_str!("../../../config/base/streams/outdoor-weather/config.json");
        let config: StreamConfig = serde_json::from_str(content).unwrap();

        assert_eq!(config.stream_id, "outdoor-weather");
        assert_eq!(config.retention_days, Some(90));
        assert!(config.enabled);
        assert!(!config.fields.is_empty());

        let etl = config.silver_etl.as_ref().unwrap();
        assert!(etl.enabled);
        assert_eq!(
            etl.target_table.as_deref(),
            Some("silver.weather_observations")
        );
        assert!(!etl.field_mappings.is_empty());
    }

    #[test]
    fn test_parse_real_air_quality_config() {
        // Verify that our StreamConfig struct can parse the real air-quality config.
        let content = include_str!("../../../config/base/streams/air-quality/config.json");
        let config: StreamConfig = serde_json::from_str(content).unwrap();

        assert_eq!(config.stream_id, "air-quality");
        assert!(config.enabled);

        let etl = config.silver_etl.as_ref().unwrap();
        assert_eq!(
            etl.target_table.as_deref(),
            Some("silver.air_quality_observations")
        );
    }

    // -----------------------------------------------------------------------
    // DomainConfig tests
    // -----------------------------------------------------------------------

    fn write_domain_config(domains_dir: &Path, domain_id: &str, content: &str) {
        let domain_dir = domains_dir.join(domain_id);
        std::fs::create_dir_all(&domain_dir).unwrap();
        let mut f = std::fs::File::create(domain_dir.join("domain.json")).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_parse_real_domain_config() {
        let content =
            include_str!("../../../config/domains/indoor-air-quality/domain.json");
        let config: DomainConfig = serde_json::from_str(content).unwrap();

        assert_eq!(config.id, "indoor-air-quality");
        assert_eq!(
            config.description.as_deref(),
            Some("Maintain healthy indoor air quality")
        );
        assert_eq!(config.streams.len(), 4);
        assert_eq!(config.objectives.len(), 6);
        assert!(config.alignment.is_some());
        assert!(config.events.is_some());
    }

    #[test]
    fn test_domain_config_no_constraints() {
        let content =
            include_str!("../../../config/domains/indoor-air-quality/domain.json");
        let config: DomainConfig = serde_json::from_str(content).unwrap();

        // The real indoor-air-quality domain has no constraints
        assert!(config.constraints.is_empty());
    }

    #[test]
    fn test_discover_domain_ids() {
        let tmp = TempDir::new().unwrap();
        let domains_dir = tmp.path().join("domains");

        write_domain_config(&domains_dir, "alpha-domain", r#"{"id":"alpha-domain"}"#);
        write_domain_config(&domains_dir, "beta-domain", r#"{"id":"beta-domain"}"#);

        let loader = FileSystemConfigLoader::new(
            tmp.path().join("streams"),
            tmp.path().join("dimensions"),
        )
        .with_domains_dir(&domains_dir);

        let ids = loader.discover_domain_ids().unwrap();
        assert_eq!(ids, vec!["alpha-domain", "beta-domain"]);
    }

    #[test]
    fn test_load_domain_configs() {
        let tmp = TempDir::new().unwrap();
        let domains_dir = tmp.path().join("domains");

        // Copy real indoor-air-quality config into the tempdir
        let real_content =
            include_str!("../../../config/domains/indoor-air-quality/domain.json");
        write_domain_config(&domains_dir, "indoor-air-quality", real_content);

        let loader = FileSystemConfigLoader::new(
            tmp.path().join("streams"),
            tmp.path().join("dimensions"),
        )
        .with_domains_dir(&domains_dir);

        let configs = loader.load_domain_configs().unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].id, "indoor-air-quality");
        assert_eq!(configs[0].streams.len(), 4);
        assert_eq!(configs[0].objectives.len(), 6);
    }
}
