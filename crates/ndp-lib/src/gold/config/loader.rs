//! Configuration loading for Gold DDL generation
//!
//! Loads stream configurations from the file system.

use crate::gold::config::domain::DomainConfig;
use crate::gold::config::types::StreamConfig;
use crate::gold::error::{GoldDdlError, Result};
use std::path::{Path, PathBuf};

/// Trait for loading configurations
pub trait ConfigLoader: Send + Sync {
    /// Load stream configuration by stream ID
    fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig>;

    /// Load domain configuration by domain ID
    fn load_domain_config(&self, domain_id: &str) -> Result<DomainConfig>;
}

/// File system configuration loader
#[derive(Clone)]
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
            .join("domain.json")
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
            serde_json::from_str(&content).map_err(|e| GoldDdlError::ConfigParseError {
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

    // ========== Phase A (FE-002): JSON Domain Config Tests ==========

    fn create_test_domain_config(dir: &Path, domain_id: &str, content: &str) {
        let domain_dir = dir.join("domains").join(domain_id);
        std::fs::create_dir_all(&domain_dir).unwrap();
        let config_path = domain_dir.join("domain.json");
        let mut file = std::fs::File::create(config_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_load_domain_config_from_json() {
        let temp_dir = TempDir::new().unwrap();
        let config_json = r#"{
            "id": "test-domain",
            "description": "Test domain for JSON loading",
            "streams": [
                {
                    "stream_id": "test-stream",
                    "alias": "test",
                    "role": "primary"
                }
            ],
            "alignment": {
                "view_name": "test_aligned",
                "granularity": "1 hour",
                "join_strategy": "full_outer"
            },
            "objectives": []
        }"#;

        create_test_domain_config(temp_dir.path(), "test-domain", config_json);

        let loader = FileSystemConfigLoader::new(temp_dir.path());
        let config = loader.load_domain_config("test-domain").unwrap();

        assert_eq!(config.id, "test-domain");
        assert_eq!(config.description, "Test domain for JSON loading");
        assert_eq!(config.streams.len(), 1);
        assert_eq!(config.streams[0].stream_id, "test-stream");
        assert_eq!(config.alignment.view_name, "test_aligned");
    }

    #[test]
    fn test_load_domain_config_json_not_found_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let loader = FileSystemConfigLoader::new(temp_dir.path());

        let result = loader.load_domain_config("nonexistent-domain");

        assert!(result.is_err());
        match result.unwrap_err() {
            GoldDdlError::ConfigNotFound { path } => {
                assert!(path.contains("nonexistent-domain"));
                assert!(path.contains("domain.json"));
            }
            _ => panic!("Expected ConfigNotFound error"),
        }
    }

    #[test]
    fn test_load_domain_config_json_parse_error() {
        let temp_dir = TempDir::new().unwrap();
        create_test_domain_config(temp_dir.path(), "bad-domain", "{ invalid json }");

        let loader = FileSystemConfigLoader::new(temp_dir.path());
        let result = loader.load_domain_config("bad-domain");

        assert!(result.is_err());
        match result.unwrap_err() {
            GoldDdlError::ConfigParseError { message } => {
                assert!(message.contains("bad-domain"));
            }
            _ => panic!("Expected ConfigParseError"),
        }
    }

    // ========== UA-004: Config path returns JSON extension ==========
    #[test]
    fn test_domain_config_path_returns_json_extension() {
        let temp_dir = TempDir::new().unwrap();
        let loader = FileSystemConfigLoader::new(temp_dir.path());

        let path = loader.domain_config_path("test-domain");

        assert!(
            path.to_string_lossy().ends_with("domain.json"),
            "Expected path to end with domain.json, got: {:?}",
            path
        );
    }

    // ========== UA-005: Preserves stream order ==========
    #[test]
    fn test_load_domain_config_preserves_stream_order() {
        let temp_dir = TempDir::new().unwrap();
        let config_json = r#"{
            "id": "ordered-domain",
            "streams": [
                {"stream_id": "first", "alias": "a", "role": "primary"},
                {"stream_id": "second", "alias": "b", "role": "context"},
                {"stream_id": "third", "alias": "c", "role": "actuator"}
            ],
            "alignment": {
                "view_name": "test_aligned",
                "granularity": "1 hour",
                "join_strategy": "full_outer"
            }
        }"#;

        create_test_domain_config(temp_dir.path(), "ordered-domain", config_json);

        let loader = FileSystemConfigLoader::new(temp_dir.path());
        let config = loader.load_domain_config("ordered-domain").unwrap();

        assert_eq!(config.streams.len(), 3);
        assert_eq!(config.streams[0].stream_id, "first");
        assert_eq!(config.streams[1].stream_id, "second");
        assert_eq!(config.streams[2].stream_id, "third");
    }

    // ========== UA-006: Handles null_handling field ==========
    #[test]
    fn test_load_domain_config_handles_null_handling() {
        let temp_dir = TempDir::new().unwrap();
        let config_json = r#"{
            "id": "null-handling-domain",
            "streams": [
                {"stream_id": "s1", "alias": "a", "role": "primary", "null_handling": "carry_forward"},
                {"stream_id": "s2", "alias": "b", "role": "context"}
            ],
            "alignment": {
                "view_name": "test_aligned",
                "granularity": "1 hour",
                "join_strategy": "full_outer"
            }
        }"#;

        create_test_domain_config(temp_dir.path(), "null-handling-domain", config_json);

        let loader = FileSystemConfigLoader::new(temp_dir.path());
        let config = loader.load_domain_config("null-handling-domain").unwrap();

        use crate::gold::config::domain::NullHandling;
        assert_eq!(
            config.streams[0].null_handling,
            Some(NullHandling::CarryForward)
        );
        assert_eq!(config.streams[1].null_handling, None);
    }

    // ========== UA-007: Handles objectives ==========
    #[test]
    fn test_load_domain_config_handles_objectives() {
        let temp_dir = TempDir::new().unwrap();
        let config_json = r#"{
            "id": "objectives-domain",
            "streams": [
                {"stream_id": "s1", "alias": "a", "role": "primary"}
            ],
            "alignment": {
                "view_name": "test_aligned",
                "granularity": "1 hour",
                "join_strategy": "full_outer"
            },
            "objectives": [
                {
                    "id": "obj1",
                    "description": "Test objective",
                    "target": {
                        "stream": "s1",
                        "metric": "value",
                        "condition": "<",
                        "threshold": 100
                    },
                    "priority": "high"
                },
                {
                    "id": "obj2",
                    "target": {
                        "stream": "s1",
                        "metric": "count",
                        "condition": ">=",
                        "threshold": 10
                    }
                }
            ]
        }"#;

        create_test_domain_config(temp_dir.path(), "objectives-domain", config_json);

        let loader = FileSystemConfigLoader::new(temp_dir.path());
        let config = loader.load_domain_config("objectives-domain").unwrap();

        assert_eq!(config.objectives.len(), 2);
        assert_eq!(config.objectives[0].id, "obj1");
        assert_eq!(config.objectives[0].target.threshold, 100.0);
        assert_eq!(config.objectives[1].id, "obj2");
    }

    // ========== UA-008: Handles alignment config ==========
    #[test]
    fn test_load_domain_config_handles_alignment() {
        let temp_dir = TempDir::new().unwrap();
        let config_json = r#"{
            "id": "alignment-domain",
            "streams": [
                {"stream_id": "s1", "alias": "a", "role": "primary"}
            ],
            "alignment": {
                "view_name": "custom_view_name",
                "granularity": "15 minutes",
                "join_strategy": "left",
                "null_handling": "interpolate"
            }
        }"#;

        create_test_domain_config(temp_dir.path(), "alignment-domain", config_json);

        let loader = FileSystemConfigLoader::new(temp_dir.path());
        let config = loader.load_domain_config("alignment-domain").unwrap();

        use crate::gold::config::domain::{JoinStrategy, NullHandling};
        assert_eq!(config.alignment.view_name, "custom_view_name");
        assert_eq!(config.alignment.granularity, "15 minutes");
        assert_eq!(config.alignment.join_strategy, JoinStrategy::Left);
        assert_eq!(config.alignment.null_handling, NullHandling::Interpolate);
    }

    // ========== UA-009: JSON preserves string escapes ==========
    #[test]
    fn test_json_preserves_string_escapes() {
        let temp_dir = TempDir::new().unwrap();
        let config_json = r#"{
            "id": "escape-test",
            "description": "Test with \"quotes\" and \\ backslash",
            "streams": [
                {"stream_id": "s1", "alias": "a", "role": "primary"}
            ],
            "alignment": {
                "view_name": "test_aligned",
                "granularity": "1 hour",
                "join_strategy": "full_outer"
            }
        }"#;

        create_test_domain_config(temp_dir.path(), "escape-test", config_json);

        let loader = FileSystemConfigLoader::new(temp_dir.path());
        let config = loader.load_domain_config("escape-test").unwrap();

        assert_eq!(config.description, r#"Test with "quotes" and \ backslash"#);
    }

    // ========== UA-010: JSON handles unicode ==========
    #[test]
    fn test_json_handles_unicode() {
        let temp_dir = TempDir::new().unwrap();
        let config_json = r#"{
            "id": "unicode-test",
            "description": "Temperature in \u00B0C, PM2.5 in \u03BCg/m\u00B3",
            "streams": [
                {"stream_id": "s1", "alias": "a", "role": "primary"}
            ],
            "alignment": {
                "view_name": "test_aligned",
                "granularity": "1 hour",
                "join_strategy": "full_outer"
            }
        }"#;

        create_test_domain_config(temp_dir.path(), "unicode-test", config_json);

        let loader = FileSystemConfigLoader::new(temp_dir.path());
        let config = loader.load_domain_config("unicode-test").unwrap();

        // Unicode escapes should be converted
        assert!(config.description.contains("C"));
        assert!(config.description.contains("g/m"));
    }

    // ========== UA-011: JSON numeric precision ==========
    #[test]
    fn test_json_numeric_precision() {
        let temp_dir = TempDir::new().unwrap();
        let config_json = r#"{
            "id": "numeric-test",
            "streams": [
                {"stream_id": "s1", "alias": "a", "role": "primary"}
            ],
            "alignment": {
                "view_name": "test_aligned",
                "granularity": "1 hour",
                "join_strategy": "full_outer"
            },
            "objectives": [
                {
                    "id": "precise",
                    "target": {
                        "stream": "s1",
                        "metric": "value",
                        "condition": "<",
                        "threshold": 123.456789
                    }
                }
            ]
        }"#;

        create_test_domain_config(temp_dir.path(), "numeric-test", config_json);

        let loader = FileSystemConfigLoader::new(temp_dir.path());
        let config = loader.load_domain_config("numeric-test").unwrap();

        assert!((config.objectives[0].target.threshold - 123.456789).abs() < 0.0001);
    }

    // ========== UA-012: JSON empty arrays ==========
    #[test]
    fn test_json_empty_arrays() {
        let temp_dir = TempDir::new().unwrap();
        let config_json = r#"{
            "id": "empty-arrays-test",
            "streams": [
                {"stream_id": "s1", "alias": "a", "role": "primary"}
            ],
            "alignment": {
                "view_name": "test_aligned",
                "granularity": "1 hour",
                "join_strategy": "full_outer"
            },
            "objectives": []
        }"#;

        create_test_domain_config(temp_dir.path(), "empty-arrays-test", config_json);

        let loader = FileSystemConfigLoader::new(temp_dir.path());
        let config = loader.load_domain_config("empty-arrays-test").unwrap();

        assert!(config.objectives.is_empty());
    }

    // ========== UA-013: JSON optional fields absent ==========
    #[test]
    fn test_json_optional_fields_absent() {
        let temp_dir = TempDir::new().unwrap();
        let config_json = r#"{
            "id": "minimal-domain",
            "streams": [
                {"stream_id": "s1", "alias": "a", "role": "primary"}
            ],
            "alignment": {
                "view_name": "test_aligned",
                "granularity": "1 hour",
                "join_strategy": "full_outer"
            }
        }"#;

        create_test_domain_config(temp_dir.path(), "minimal-domain", config_json);

        let loader = FileSystemConfigLoader::new(temp_dir.path());
        let config = loader.load_domain_config("minimal-domain").unwrap();

        // Optional fields should default
        assert!(config.description.is_empty() || config.description == "");
        assert!(config.objectives.is_empty());
    }

    // ========== UA-014: JSON extra fields ignored ==========
    #[test]
    fn test_json_extra_fields_ignored() {
        let temp_dir = TempDir::new().unwrap();
        let config_json = r#"{
            "id": "extra-fields-domain",
            "extra_field": "this should be ignored",
            "another_extra": 12345,
            "streams": [
                {"stream_id": "s1", "alias": "a", "role": "primary", "extra_stream_field": true}
            ],
            "alignment": {
                "view_name": "test_aligned",
                "granularity": "1 hour",
                "join_strategy": "full_outer"
            }
        }"#;

        create_test_domain_config(temp_dir.path(), "extra-fields-domain", config_json);

        let loader = FileSystemConfigLoader::new(temp_dir.path());
        let result = loader.load_domain_config("extra-fields-domain");

        // Should succeed even with extra fields
        assert!(
            result.is_ok(),
            "JSON with extra fields should still parse: {:?}",
            result.err()
        );
    }

    // ========== UA-015: JSON field ordering ==========
    #[test]
    fn test_json_field_ordering() {
        let temp_dir = TempDir::new().unwrap();
        // Fields in non-standard order
        let config_json = r#"{
            "alignment": {
                "join_strategy": "full_outer",
                "view_name": "test_aligned",
                "granularity": "1 hour"
            },
            "streams": [
                {"role": "primary", "stream_id": "s1", "alias": "a"}
            ],
            "id": "reordered-domain",
            "description": "Fields in different order"
        }"#;

        create_test_domain_config(temp_dir.path(), "reordered-domain", config_json);

        let loader = FileSystemConfigLoader::new(temp_dir.path());
        let config = loader.load_domain_config("reordered-domain").unwrap();

        // All fields should be parsed correctly regardless of order
        assert_eq!(config.id, "reordered-domain");
        assert_eq!(config.streams[0].stream_id, "s1");
        assert_eq!(config.alignment.view_name, "test_aligned");
    }
}
