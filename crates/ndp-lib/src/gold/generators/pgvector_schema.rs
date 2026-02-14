//! PgVector schema DDL generator
//!
//! Generates DDL for intelligence layer tables from IntelligenceConfig.
//! All schema is config-driven: vector dimensions derived from field count,
//! graph tables conditionally generated, no hardcoded SQL files.

use crate::gold::config::Action;
use crate::gold::embeddings::config::IntelligenceConfig;

/// Configuration for PgVector schema generation.
pub struct PgVectorSchemaConfig {
    /// The intelligence configuration to generate schema from
    pub intelligence: IntelligenceConfig,
    /// Whether to include graph tables (gold.graph_nodes, gold.graph_edges)
    pub include_graph_tables: bool,
    /// Whether to include the reasoning bank table
    pub include_reasoning_bank: bool,
}

impl PgVectorSchemaConfig {
    /// Create a config from IntelligenceConfig with default options.
    pub fn new(intelligence: IntelligenceConfig) -> Self {
        Self {
            intelligence,
            include_graph_tables: true,
            include_reasoning_bank: true,
        }
    }

    /// Set whether to include graph tables.
    pub fn with_graph_tables(mut self, include: bool) -> Self {
        self.include_graph_tables = include;
        self
    }

    /// Set whether to include the reasoning bank.
    pub fn with_reasoning_bank(mut self, include: bool) -> Self {
        self.include_reasoning_bank = include;
        self
    }
}

/// Generates DDL for pgvector-based intelligence tables.
///
/// All DDL is derived from config, not hardcoded. The generator reads
/// `IntelligenceConfig` and produces appropriate DDL based on:
/// - Vector dimensions from field count
/// - Conditional graph tables
/// - Conditional reasoning bank
pub struct PgVectorSchemaGenerator;

impl PgVectorSchemaGenerator {
    /// Generate DDL for the intelligence schema.
    pub fn generate(config: &PgVectorSchemaConfig, action: Action) -> String {
        let dimensions = config.intelligence.embedding.fields.total_dimensions();
        let mut ddl = String::new();

        match action {
            Action::Sync => {
                Self::append_extension(&mut ddl);
                Self::append_schema(&mut ddl);
                Self::append_metric_embeddings(&mut ddl, dimensions);
                Self::append_predictions(&mut ddl);
                if config.include_graph_tables {
                    Self::append_graph_nodes(&mut ddl);
                    Self::append_graph_edges(&mut ddl);
                }
                if config.include_reasoning_bank {
                    Self::append_reasoning_bank(&mut ddl);
                }
            }
            Action::Recreate => {
                // Drop in reverse dependency order
                if config.include_reasoning_bank {
                    ddl.push_str("DROP TABLE IF EXISTS gold.reasoning_bank CASCADE;\n");
                }
                if config.include_graph_tables {
                    ddl.push_str("DROP TABLE IF EXISTS gold.graph_edges CASCADE;\n");
                    ddl.push_str("DROP TABLE IF EXISTS gold.graph_nodes CASCADE;\n");
                }
                ddl.push_str("DROP TABLE IF EXISTS gold.predictions CASCADE;\n");
                ddl.push_str("DROP TABLE IF EXISTS gold.metric_embeddings CASCADE;\n");
                ddl.push('\n');
                // Then create
                Self::append_extension(&mut ddl);
                Self::append_schema(&mut ddl);
                Self::append_metric_embeddings(&mut ddl, dimensions);
                Self::append_predictions(&mut ddl);
                if config.include_graph_tables {
                    Self::append_graph_nodes(&mut ddl);
                    Self::append_graph_edges(&mut ddl);
                }
                if config.include_reasoning_bank {
                    Self::append_reasoning_bank(&mut ddl);
                }
            }
        }

        ddl
    }

    fn append_extension(ddl: &mut String) {
        ddl.push_str("-- pgvector extension\n");
        ddl.push_str("CREATE EXTENSION IF NOT EXISTS vector;\n\n");
    }

    fn append_schema(ddl: &mut String) {
        ddl.push_str("CREATE SCHEMA IF NOT EXISTS gold;\n\n");
    }

    fn append_metric_embeddings(ddl: &mut String, dimensions: usize) {
        ddl.push_str("-- Metric embeddings hypertable\n");
        ddl.push_str("CREATE TABLE IF NOT EXISTS gold.metric_embeddings (\n");
        ddl.push_str("    bucket          TIMESTAMPTZ NOT NULL,\n");
        ddl.push_str("    domain_id       TEXT NOT NULL,\n");
        ddl.push_str(&format!(
            "    embedding       vector({}),\n",
            dimensions
        ));
        ddl.push_str("    dimensions      INTEGER NOT NULL,\n");
        ddl.push_str("    metadata        JSONB DEFAULT '{}',\n");
        ddl.push_str("    created_at      TIMESTAMPTZ DEFAULT NOW(),\n");
        ddl.push_str("    PRIMARY KEY (bucket, domain_id)\n");
        ddl.push_str(");\n\n");
        ddl.push_str("SELECT create_hypertable('gold.metric_embeddings', 'bucket',\n");
        ddl.push_str("    chunk_time_interval => INTERVAL '7 days',\n");
        ddl.push_str("    if_not_exists => TRUE);\n\n");
        ddl.push_str("CREATE INDEX IF NOT EXISTS idx_metric_embeddings_domain\n");
        ddl.push_str("    ON gold.metric_embeddings(domain_id, bucket DESC);\n\n");
    }

    fn append_predictions(ddl: &mut String) {
        ddl.push_str("-- Predictions hypertable\n");
        ddl.push_str("CREATE TABLE IF NOT EXISTS gold.predictions (\n");
        ddl.push_str("    id              BIGSERIAL,\n");
        ddl.push_str("    bucket          TIMESTAMPTZ NOT NULL,\n");
        ddl.push_str("    domain_id       TEXT NOT NULL,\n");
        ddl.push_str("    metric          TEXT NOT NULL,\n");
        ddl.push_str("    horizon         INTERVAL NOT NULL,\n");
        ddl.push_str("    predicted_value DOUBLE PRECISION,\n");
        ddl.push_str("    predicted_breach BOOLEAN,\n");
        ddl.push_str("    confidence      DOUBLE PRECISION,\n");
        ddl.push_str("    k_neighbors     INTEGER,\n");
        ddl.push_str("    k_supporting    INTEGER,\n");
        ddl.push_str("    actual_value    DOUBLE PRECISION,\n");
        ddl.push_str("    actual_breach   BOOLEAN,\n");
        ddl.push_str("    correct         BOOLEAN,\n");
        ddl.push_str("    evaluated_at    TIMESTAMPTZ,\n");
        ddl.push_str("    created_at      TIMESTAMPTZ DEFAULT NOW(),\n");
        ddl.push_str("    PRIMARY KEY (id, bucket)\n");
        ddl.push_str(");\n\n");
        ddl.push_str("SELECT create_hypertable('gold.predictions', 'bucket',\n");
        ddl.push_str("    chunk_time_interval => INTERVAL '30 days',\n");
        ddl.push_str("    if_not_exists => TRUE);\n\n");
        ddl.push_str("CREATE INDEX IF NOT EXISTS idx_predictions_domain_metric\n");
        ddl.push_str("    ON gold.predictions(domain_id, metric, bucket DESC);\n\n");
        ddl.push_str("CREATE INDEX IF NOT EXISTS idx_predictions_pending\n");
        ddl.push_str("    ON gold.predictions(domain_id, bucket)\n");
        ddl.push_str("    WHERE actual_value IS NULL;\n\n");
    }

    fn append_graph_nodes(ddl: &mut String) {
        ddl.push_str("-- Graph nodes\n");
        ddl.push_str("CREATE TABLE IF NOT EXISTS gold.graph_nodes (\n");
        ddl.push_str("    id              TEXT PRIMARY KEY,\n");
        ddl.push_str("    node_type       TEXT NOT NULL,\n");
        ddl.push_str("    properties      JSONB DEFAULT '{}',\n");
        ddl.push_str("    created_at      TIMESTAMPTZ DEFAULT NOW()\n");
        ddl.push_str(");\n\n");
        ddl.push_str("CREATE INDEX IF NOT EXISTS idx_graph_nodes_type\n");
        ddl.push_str("    ON gold.graph_nodes(node_type);\n\n");
    }

    fn append_graph_edges(ddl: &mut String) {
        ddl.push_str("-- Graph edges\n");
        ddl.push_str("CREATE TABLE IF NOT EXISTS gold.graph_edges (\n");
        ddl.push_str("    id              SERIAL PRIMARY KEY,\n");
        ddl.push_str("    source_id       TEXT NOT NULL REFERENCES gold.graph_nodes(id),\n");
        ddl.push_str("    target_id       TEXT NOT NULL REFERENCES gold.graph_nodes(id),\n");
        ddl.push_str("    edge_type       TEXT NOT NULL,\n");
        ddl.push_str("    weight          DOUBLE PRECISION DEFAULT 1.0,\n");
        ddl.push_str("    properties      JSONB DEFAULT '{}',\n");
        ddl.push_str("    created_at      TIMESTAMPTZ DEFAULT NOW()\n");
        ddl.push_str(");\n\n");
        ddl.push_str("CREATE INDEX IF NOT EXISTS idx_graph_edges_source\n");
        ddl.push_str("    ON gold.graph_edges(source_id, edge_type);\n");
        ddl.push_str("CREATE INDEX IF NOT EXISTS idx_graph_edges_target\n");
        ddl.push_str("    ON gold.graph_edges(target_id, edge_type);\n\n");
    }

    fn append_reasoning_bank(ddl: &mut String) {
        ddl.push_str("-- Reasoning bank (V1.3 prep)\n");
        ddl.push_str("CREATE TABLE IF NOT EXISTS gold.reasoning_bank (\n");
        ddl.push_str("    id              SERIAL PRIMARY KEY,\n");
        ddl.push_str("    domain_id       TEXT NOT NULL,\n");
        ddl.push_str("    adapter_name    TEXT NOT NULL,\n");
        ddl.push_str("    adapter_blob    BYTEA,\n");
        ddl.push_str("    ewc_fisher      BYTEA,\n");
        ddl.push_str("    created_at      TIMESTAMPTZ DEFAULT NOW(),\n");
        ddl.push_str("    performance     JSONB DEFAULT '{}'\n");
        ddl.push_str(");\n\n");
        ddl.push_str("CREATE INDEX IF NOT EXISTS idx_reasoning_bank_domain\n");
        ddl.push_str("    ON gold.reasoning_bank(domain_id);\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gold::embeddings::config::*;

    fn test_config() -> PgVectorSchemaConfig {
        let intel = IntelligenceConfig {
            enabled: true,
            embedding: EmbeddingConfig {
                embedding_type: EmbeddingType::Metric,
                fields: EmbeddingFieldsConfig {
                    temporal: vec![
                        "hour_sin".to_string(),
                        "hour_cos".to_string(),
                        "is_weekend".to_string(),
                    ],
                    direct: vec![
                        DirectFieldConfig {
                            field: "pm25_mean".to_string(),
                            null_strategy: NullStrategyConfig::Zero,
                        },
                        DirectFieldConfig {
                            field: "co2_mean".to_string(),
                            null_strategy: NullStrategyConfig::LastKnown,
                        },
                        DirectFieldConfig {
                            field: "temperature_c_mean".to_string(),
                            null_strategy: NullStrategyConfig::Mean,
                        },
                    ],
                    derived: vec!["pm25_co2_ratio".to_string()],
                },
            },
            search: SearchConfig {
                k: 10,
                min_similarity: 0.85,
                prediction_horizons: vec!["1 hour".to_string()],
            },
            anomaly: None,
        };
        PgVectorSchemaConfig::new(intel)
    }

    #[test]
    fn test_sync_output_contains_if_not_exists() {
        let config = test_config();
        let ddl = PgVectorSchemaGenerator::generate(&config, Action::Sync);
        assert!(
            ddl.contains("IF NOT EXISTS"),
            "Sync DDL should contain IF NOT EXISTS"
        );
    }

    #[test]
    fn test_extension_ddl() {
        let config = test_config();
        let ddl = PgVectorSchemaGenerator::generate(&config, Action::Sync);
        assert!(
            ddl.contains("CREATE EXTENSION IF NOT EXISTS vector"),
            "DDL should create pgvector extension"
        );
    }

    #[test]
    fn test_hypertable_calls() {
        let config = test_config();
        let ddl = PgVectorSchemaGenerator::generate(&config, Action::Sync);
        assert!(
            ddl.contains("create_hypertable('gold.metric_embeddings'"),
            "DDL should create hypertable for metric_embeddings"
        );
        assert!(
            ddl.contains("create_hypertable('gold.predictions'"),
            "DDL should create hypertable for predictions"
        );
    }

    #[test]
    fn test_graph_tables_included() {
        let config = test_config().with_graph_tables(true);
        let ddl = PgVectorSchemaGenerator::generate(&config, Action::Sync);
        assert!(
            ddl.contains("gold.graph_nodes"),
            "DDL should include graph_nodes when flag=true"
        );
        assert!(
            ddl.contains("gold.graph_edges"),
            "DDL should include graph_edges when flag=true"
        );
    }

    #[test]
    fn test_graph_tables_excluded() {
        let config = test_config().with_graph_tables(false);
        let ddl = PgVectorSchemaGenerator::generate(&config, Action::Sync);
        assert!(
            !ddl.contains("gold.graph_nodes"),
            "DDL should not include graph_nodes when flag=false"
        );
        assert!(
            !ddl.contains("gold.graph_edges"),
            "DDL should not include graph_edges when flag=false"
        );
    }

    #[test]
    fn test_predictions_has_created_at() {
        let config = test_config();
        let ddl = PgVectorSchemaGenerator::generate(&config, Action::Sync);
        // The predictions table should contain created_at
        let predictions_section = ddl
            .split("-- Predictions")
            .nth(1)
            .expect("Should have predictions section");
        assert!(
            predictions_section.contains("created_at      TIMESTAMPTZ DEFAULT NOW()"),
            "Predictions table should have created_at column"
        );
    }

    #[test]
    fn test_reasoning_bank() {
        let config = test_config();
        let ddl = PgVectorSchemaGenerator::generate(&config, Action::Sync);
        assert!(
            ddl.contains("adapter_blob    BYTEA"),
            "DDL should include adapter_blob BYTEA"
        );
        assert!(
            ddl.contains("ewc_fisher      BYTEA"),
            "DDL should include ewc_fisher BYTEA"
        );
    }

    #[test]
    fn test_reasoning_bank_excluded() {
        let config = test_config().with_reasoning_bank(false);
        let ddl = PgVectorSchemaGenerator::generate(&config, Action::Sync);
        assert!(
            !ddl.contains("reasoning_bank"),
            "DDL should not include reasoning_bank when flag=false"
        );
    }

    #[test]
    fn test_vector_dimensions_from_config() {
        let config = test_config();
        // 3 temporal + 3 direct + 1 derived = 7
        let ddl = PgVectorSchemaGenerator::generate(&config, Action::Sync);
        assert!(
            ddl.contains("vector(7)"),
            "DDL should use vector(7) based on field count, DDL: {}",
            ddl
        );
    }

    #[test]
    fn test_pending_outcomes_filter() {
        let config = test_config();
        let ddl = PgVectorSchemaGenerator::generate(&config, Action::Sync);
        assert!(
            ddl.contains("WHERE actual_value IS NULL"),
            "DDL should include pending outcomes partial index"
        );
    }

    #[test]
    fn test_recreate_includes_drops() {
        let config = test_config();
        let ddl = PgVectorSchemaGenerator::generate(&config, Action::Recreate);
        assert!(
            ddl.contains("DROP TABLE IF EXISTS gold.metric_embeddings CASCADE"),
            "Recreate should DROP metric_embeddings"
        );
        assert!(
            ddl.contains("DROP TABLE IF EXISTS gold.predictions CASCADE"),
            "Recreate should DROP predictions"
        );
    }

    #[test]
    fn test_schema_creation() {
        let config = test_config();
        let ddl = PgVectorSchemaGenerator::generate(&config, Action::Sync);
        assert!(
            ddl.contains("CREATE SCHEMA IF NOT EXISTS gold"),
            "DDL should create gold schema"
        );
    }
}
