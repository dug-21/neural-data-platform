//! Gold text view generator (dp-023)
//!
//! Generates per-domain VIEWs over Silver text/jsonb columns.
//! The VIEW uses an unpivoted schema (source_stream, field_name, value)
//! with DISTINCT ON to return the latest text value per stream per field.
//!
//! This is a VIEW, not a MATERIALIZED VIEW -- no refresh policy needed.
//! The intelligence engine reads this view on existing `gold_refresh` events.

use crate::gold::config::{Action, ConfigLoader, DomainConfig};
use crate::gold::error::Result;

/// Information about a text field discovered in a stream's Silver ETL config
#[derive(Debug, Clone)]
struct TextFieldInfo {
    /// Stream ID (e.g., "nws-forecast-hourly")
    stream_id: String,
    /// Silver table name (e.g., "silver.nws_forecast_hourly")
    silver_table: String,
    /// Column name in Silver table (e.g., "short_forecast")
    column_name: String,
    /// PostgreSQL column type ("text", "jsonb", "varchar", "text[]")
    field_type: String,
    /// Timestamp column name (e.g., "observation_time")
    timestamp_column: String,
}

/// Generator for Gold text views
///
/// Scans domain streams for text/jsonb field mappings and generates
/// a VIEW that unions all text fields into a single unpivoted table.
pub struct TextViewGenerator<L: ConfigLoader> {
    config_loader: L,
}

impl<L: ConfigLoader> TextViewGenerator<L> {
    /// Create a new generator with the given config loader
    pub fn new(config_loader: L) -> Self {
        Self { config_loader }
    }

    /// Generate text view DDL for a domain
    ///
    /// Returns SQL to CREATE OR REPLACE VIEW gold.{domain}_text
    /// with columns: time, source_stream, field_name, value
    pub fn generate(&self, domain_id: &str, action: Action) -> Result<String> {
        let domain_config = self.config_loader.load_domain_config(domain_id)?;
        let text_fields = self.discover_text_fields(&domain_config)?;

        if text_fields.is_empty() {
            return Ok(format!(
                "-- No text fields found for domain {}\n",
                domain_id
            ));
        }

        let view_name = format!("gold.{}_text", domain_id.replace('-', "_"));

        // Build UNION ALL subqueries, one per text field
        let mut subqueries = Vec::new();
        for field in &text_fields {
            // Cast jsonb to text for uniform view schema
            let value_expr = if field.field_type == "jsonb" {
                format!("{}::text", field.column_name)
            } else {
                field.column_name.clone()
            };

            subqueries.push(format!(
                "SELECT {ts} AS time, '{stream}' AS source_stream, \
                 '{col}' AS field_name, {value_expr} AS value \
                 FROM {table} WHERE {col} IS NOT NULL",
                ts = field.timestamp_column,
                stream = field.stream_id.replace('-', "_"),
                col = field.column_name,
                value_expr = value_expr,
                table = field.silver_table,
            ));
        }

        let union_query = subqueries.join("\n    UNION ALL\n    ");

        let drop_clause = match action {
            Action::Recreate => format!("DROP VIEW IF EXISTS {} CASCADE;\n", view_name),
            _ => String::new(),
        };

        let sql = format!(
            "{drop}CREATE OR REPLACE VIEW {view} AS\n\
             SELECT DISTINCT ON (source_stream, field_name)\n\
                 t.time,\n\
                 t.source_stream,\n\
                 t.field_name,\n\
                 t.value\n\
             FROM (\n\
                 {union}\n\
             ) t\n\
             ORDER BY t.source_stream, t.field_name, t.time DESC;\n\n\
             COMMENT ON VIEW {view} IS 'Latest text field values for domain {domain} (dp-023, config-driven)';\n",
            drop = drop_clause,
            view = view_name,
            union = union_query,
            domain = domain_id,
        );

        Ok(sql)
    }

    /// Discover text/jsonb fields across all streams in a domain
    fn discover_text_fields(&self, domain_config: &DomainConfig) -> Result<Vec<TextFieldInfo>> {
        let mut text_fields = Vec::new();

        for stream_ref in &domain_config.streams {
            let stream_config = match self.config_loader.load_stream_config(&stream_ref.stream_id) {
                Ok(config) => config,
                Err(_) => continue, // Skip streams that can't be loaded
            };

            let silver_etl = match &stream_config.silver_etl {
                Some(etl) => etl,
                None => continue, // Skip streams without silver_etl
            };

            let timestamp_col = silver_etl
                .timestamp
                .as_ref()
                .map(|ts| ts.target_field.clone())
                .unwrap_or_else(|| "observation_time".to_string());

            let silver_table = &silver_etl.target_table;
            if silver_table.is_empty() {
                continue;
            }

            for mapping in &silver_etl.field_mappings {
                let col_type = mapping.column_type.as_str();
                if matches!(col_type, "text" | "varchar" | "jsonb" | "text[]") {
                    text_fields.push(TextFieldInfo {
                        stream_id: stream_ref.stream_id.clone(),
                        silver_table: silver_table.clone(),
                        column_name: mapping.target_column.clone(),
                        field_type: col_type.to_string(),
                        timestamp_column: timestamp_col.clone(),
                    });
                }
            }
        }

        Ok(text_fields)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gold::config::types::{
        SilverEtlConfig, SilverFieldMapping, StreamConfig, TimestampConfig,
    };
    use crate::gold::config::{
        AlignmentConfig, DomainConfig, JoinStrategy, NullHandling, StreamRef, StreamRole,
    };
    use crate::gold::error::{GoldDdlError, Result};

    /// Test config loader that returns pre-configured streams
    struct TestConfigLoader {
        streams: std::collections::HashMap<String, StreamConfig>,
    }

    impl TestConfigLoader {
        fn new() -> Self {
            Self {
                streams: std::collections::HashMap::new(),
            }
        }

        fn with_stream(mut self, id: &str, config: StreamConfig) -> Self {
            self.streams.insert(id.to_string(), config);
            self
        }
    }

    impl ConfigLoader for TestConfigLoader {
        fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig> {
            self.streams
                .get(stream_id)
                .cloned()
                .ok_or_else(|| GoldDdlError::ConfigNotFound {
                    path: stream_id.to_string(),
                })
        }

        fn load_domain_config(&self, _domain_id: &str) -> Result<DomainConfig> {
            Err(GoldDdlError::ConfigNotFound {
                path: "not used in tests".to_string(),
            })
        }
    }

    fn make_domain_config(stream_ids: Vec<&str>) -> DomainConfig {
        DomainConfig {
            id: "test-domain".to_string(),
            description: "Test domain".to_string(),
            streams: stream_ids
                .into_iter()
                .map(|id| StreamRef {
                    stream_id: id.to_string(),
                    alias: id.replace('-', "_"),
                    role: StreamRole::Primary,
                    null_handling: Some(NullHandling::Preserve),
                })
                .collect(),
            alignment: AlignmentConfig {
                view_name: "test_domain_aligned".to_string(),
                granularity: "1 hour".to_string(),
                join_strategy: JoinStrategy::FullOuter,
                null_handling: NullHandling::Preserve,
            },
            objectives: vec![],
            events: None,
            intelligence: None,
        }
    }

    #[test]
    fn test_generate_single_text_field() {
        let loader = TestConfigLoader::new().with_stream(
            "nws-forecast-hourly",
            StreamConfig {
                stream_id: "nws-forecast-hourly".to_string(),
                stream_type: None,
                fields: vec![],
                silver_etl: Some(SilverEtlConfig {
                    target_table: "silver.nws_forecast_hourly".to_string(),
                    timestamp: Some(TimestampConfig {
                        target_field: "observation_time".to_string(),
                    }),
                    field_mappings: vec![
                        SilverFieldMapping {
                            source_path: "temperature".to_string(),
                            target_column: "temperature_f".to_string(),
                            column_type: "double_precision".to_string(),
                        },
                        SilverFieldMapping {
                            source_path: "short_forecast".to_string(),
                            target_column: "short_forecast".to_string(),
                            column_type: "text".to_string(),
                        },
                    ],
                }),
                gold_etl: None,
            },
        );

        let generator = TextViewGenerator::new(loader);
        let domain = make_domain_config(vec!["nws-forecast-hourly"]);
        let text_fields = generator.discover_text_fields(&domain).unwrap();

        assert_eq!(text_fields.len(), 1);
        assert_eq!(text_fields[0].column_name, "short_forecast");
        assert_eq!(text_fields[0].field_type, "text");
    }

    #[test]
    fn test_generate_mixed_numeric_text() {
        let loader = TestConfigLoader::new().with_stream(
            "nws-forecast-hourly",
            StreamConfig {
                stream_id: "nws-forecast-hourly".to_string(),
                stream_type: None,
                fields: vec![],
                silver_etl: Some(SilverEtlConfig {
                    target_table: "silver.nws_forecast_hourly".to_string(),
                    timestamp: Some(TimestampConfig {
                        target_field: "observation_time".to_string(),
                    }),
                    field_mappings: vec![
                        SilverFieldMapping {
                            source_path: "temperature".to_string(),
                            target_column: "temperature_f".to_string(),
                            column_type: "double_precision".to_string(),
                        },
                        SilverFieldMapping {
                            source_path: "short_forecast".to_string(),
                            target_column: "short_forecast".to_string(),
                            column_type: "text".to_string(),
                        },
                        SilverFieldMapping {
                            source_path: "detailed_forecast".to_string(),
                            target_column: "detailed_forecast".to_string(),
                            column_type: "text".to_string(),
                        },
                    ],
                }),
                gold_etl: None,
            },
        );

        let generator = TextViewGenerator::new(loader);
        let domain = make_domain_config(vec!["nws-forecast-hourly"]);
        let text_fields = generator.discover_text_fields(&domain).unwrap();

        // Should find 2 text fields, not the numeric one
        assert_eq!(text_fields.len(), 2);
        assert!(text_fields.iter().all(|f| f.field_type == "text"));
        assert!(text_fields
            .iter()
            .any(|f| f.column_name == "short_forecast"));
        assert!(text_fields
            .iter()
            .any(|f| f.column_name == "detailed_forecast"));
    }

    #[test]
    fn test_generate_no_text_fields() {
        let loader = TestConfigLoaderWithDomain::new(
            make_domain_config(vec!["air-quality"]),
            vec![(
                "air-quality",
                StreamConfig {
                    stream_id: "air-quality".to_string(),
                    stream_type: None,
                    fields: vec![],
                    silver_etl: Some(SilverEtlConfig {
                        target_table: "silver.air_quality".to_string(),
                        timestamp: None,
                        field_mappings: vec![SilverFieldMapping {
                            source_path: "co2".to_string(),
                            target_column: "co2".to_string(),
                            column_type: "double_precision".to_string(),
                        }],
                    }),
                    gold_etl: None,
                },
            )],
        );

        let generator = TextViewGenerator::new(loader);
        let sql = generator.generate("test-domain", Action::Sync).unwrap();

        assert!(sql.contains("-- No text fields"));
    }

    #[test]
    fn test_generate_sql_structure() {
        let loader = TestConfigLoader::new().with_stream(
            "nws-forecast-hourly",
            StreamConfig {
                stream_id: "nws-forecast-hourly".to_string(),
                stream_type: None,
                fields: vec![],
                silver_etl: Some(SilverEtlConfig {
                    target_table: "silver.nws_forecast_hourly".to_string(),
                    timestamp: Some(TimestampConfig {
                        target_field: "observation_time".to_string(),
                    }),
                    field_mappings: vec![SilverFieldMapping {
                        source_path: "short_forecast".to_string(),
                        target_column: "short_forecast".to_string(),
                        column_type: "text".to_string(),
                    }],
                }),
                gold_etl: None,
            },
        );

        let generator = TextViewGenerator::new(loader);

        // Use a custom domain config that the test loader can handle
        let domain = make_domain_config(vec!["nws-forecast-hourly"]);
        let text_fields = generator.discover_text_fields(&domain).unwrap();
        assert_eq!(text_fields.len(), 1);

        // Test the full SQL generation by providing a domain_config directly
        // We need the loader to return the domain config too
        let loader2 = TestConfigLoaderWithDomain::new(
            make_domain_config(vec!["nws-forecast-hourly"]),
            vec![(
                "nws-forecast-hourly",
                StreamConfig {
                    stream_id: "nws-forecast-hourly".to_string(),
                    stream_type: None,
                    fields: vec![],
                    silver_etl: Some(SilverEtlConfig {
                        target_table: "silver.nws_forecast_hourly".to_string(),
                        timestamp: Some(TimestampConfig {
                            target_field: "observation_time".to_string(),
                        }),
                        field_mappings: vec![SilverFieldMapping {
                            source_path: "short_forecast".to_string(),
                            target_column: "short_forecast".to_string(),
                            column_type: "text".to_string(),
                        }],
                    }),
                    gold_etl: None,
                },
            )],
        );

        let generator2 = TextViewGenerator::new(loader2);
        let sql = generator2.generate("test-domain", Action::Sync).unwrap();

        assert!(sql.contains("CREATE OR REPLACE VIEW gold.test_domain_text"));
        assert!(sql.contains("DISTINCT ON"));
        assert!(sql.contains("source_stream"));
        assert!(sql.contains("field_name"));
        assert!(sql.contains("short_forecast"));
        assert!(sql.contains("COMMENT ON VIEW"));
        // Should NOT contain DROP when action is Sync
        assert!(!sql.contains("DROP VIEW"));
    }

    #[test]
    fn test_generate_recreate_includes_drop() {
        let loader = TestConfigLoaderWithDomain::new(
            make_domain_config(vec!["nws-forecast-hourly"]),
            vec![(
                "nws-forecast-hourly",
                StreamConfig {
                    stream_id: "nws-forecast-hourly".to_string(),
                    stream_type: None,
                    fields: vec![],
                    silver_etl: Some(SilverEtlConfig {
                        target_table: "silver.nws_forecast_hourly".to_string(),
                        timestamp: Some(TimestampConfig {
                            target_field: "observation_time".to_string(),
                        }),
                        field_mappings: vec![SilverFieldMapping {
                            source_path: "short_forecast".to_string(),
                            target_column: "short_forecast".to_string(),
                            column_type: "text".to_string(),
                        }],
                    }),
                    gold_etl: None,
                },
            )],
        );

        let generator = TextViewGenerator::new(loader);
        let sql = generator.generate("test-domain", Action::Recreate).unwrap();

        assert!(sql.contains("DROP VIEW IF EXISTS"));
        assert!(sql.contains("CREATE OR REPLACE VIEW"));
    }

    #[test]
    fn test_jsonb_field_cast_to_text() {
        let loader = TestConfigLoaderWithDomain::new(
            make_domain_config(vec!["test-stream"]),
            vec![(
                "test-stream",
                StreamConfig {
                    stream_id: "test-stream".to_string(),
                    stream_type: None,
                    fields: vec![],
                    silver_etl: Some(SilverEtlConfig {
                        target_table: "silver.test_stream".to_string(),
                        timestamp: Some(TimestampConfig {
                            target_field: "observation_time".to_string(),
                        }),
                        field_mappings: vec![SilverFieldMapping {
                            source_path: "metadata".to_string(),
                            target_column: "metadata".to_string(),
                            column_type: "jsonb".to_string(),
                        }],
                    }),
                    gold_etl: None,
                },
            )],
        );

        let generator = TextViewGenerator::new(loader);
        let sql = generator.generate("test-domain", Action::Sync).unwrap();

        // JSONB columns should be cast to text in the view
        assert!(sql.contains("metadata::text"));
    }

    /// Test config loader that also provides a domain config
    struct TestConfigLoaderWithDomain {
        domain: DomainConfig,
        streams: std::collections::HashMap<String, StreamConfig>,
    }

    impl TestConfigLoaderWithDomain {
        fn new(domain: DomainConfig, streams: Vec<(&str, StreamConfig)>) -> Self {
            Self {
                domain,
                streams: streams
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v))
                    .collect(),
            }
        }
    }

    impl ConfigLoader for TestConfigLoaderWithDomain {
        fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig> {
            self.streams
                .get(stream_id)
                .cloned()
                .ok_or_else(|| GoldDdlError::ConfigNotFound {
                    path: stream_id.to_string(),
                })
        }

        fn load_domain_config(&self, _domain_id: &str) -> Result<DomainConfig> {
            Ok(self.domain.clone())
        }
    }
}
