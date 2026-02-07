//! Classification SQL generator for v11-002
//!
//! Generates SQL to sync stream type classifications to the data dictionary.
//! This enables correlation analysis in V1.2 by categorizing streams as
//! causes vs effects.

use crate::gold::config::StreamType;
use crate::gold::error::GoldDdlError;

/// Generate classification INSERT/UPSERT SQL for a stream
///
/// # Arguments
///
/// * `stream_id` - The stream identifier (e.g., "air-quality")
/// * `stream_type` - The classification type (Observation, StateEvent, etc.)
/// * `description` - Optional description for the classification
///
/// # Returns
///
/// SQL string with UPSERT statement for stream_classification table
///
/// # Example
///
/// ```rust
/// use ndp_lib::gold::generators::classification::generate_classification_sql;
/// use ndp_lib::gold::StreamType;
///
/// let sql = generate_classification_sql("air-quality", StreamType::Observation, None);
/// assert!(sql.contains("INSERT INTO data_dictionary.stream_classification"));
/// assert!(sql.contains("'air-quality'"));
/// assert!(sql.contains("'observation'"));
/// ```
pub fn generate_classification_sql(
    stream_id: &str,
    stream_type: StreamType,
    description: Option<&str>,
) -> String {
    let role = stream_type.correlation_role();
    let null_handling = stream_type.null_handling();
    let stream_type_str = stream_type.to_string().to_lowercase();

    // Escape single quotes in description
    let desc_sql = match description {
        Some(d) => format!("'{}'", d.replace('\'', "''")),
        None => "NULL".to_string(),
    };

    format!(
        "INSERT INTO data_dictionary.stream_classification \
         (stream_id, stream_type, correlation_role, null_handling, description) \
         VALUES ('{stream_id}', '{stream_type}', '{role}', '{null_handling}', {desc}) \
         ON CONFLICT (stream_id) DO UPDATE SET \
         stream_type = EXCLUDED.stream_type, \
         correlation_role = EXCLUDED.correlation_role, \
         null_handling = EXCLUDED.null_handling, \
         description = COALESCE(EXCLUDED.description, data_dictionary.stream_classification.description), \
         updated_at = NOW();",
        stream_id = stream_id,
        stream_type = stream_type_str,
        role = role,
        null_handling = null_handling,
        desc = desc_sql
    )
}

/// Generate classification SQL for a Gold table entry
///
/// When a Gold table is created, we also record its source stream type
/// in the gold_tables data dictionary table.
///
/// # Arguments
///
/// * `table_name` - Full table name (e.g., "gold.air_quality_hourly")
/// * `object_type` - Type of object (continuous_aggregate, materialized_view, aligned_view)
/// * `source_silver_table` - Source Silver table (e.g., "silver.air_quality_observations")
/// * `source_stream_type` - Stream type of the source stream
/// * `granularity` - Time granularity (e.g., "1 hour")
/// * `description` - Optional description
///
/// # Returns
///
/// SQL string with UPSERT statement for gold_tables table
pub fn generate_gold_table_sql(
    table_name: &str,
    object_type: &str,
    source_silver_table: Option<&str>,
    source_stream_type: Option<StreamType>,
    granularity: Option<&str>,
    description: Option<&str>,
) -> String {
    let source_silver_sql = match source_silver_table {
        Some(t) => format!("'{}'", t),
        None => "NULL".to_string(),
    };

    let source_type_sql = match source_stream_type {
        Some(t) => format!("'{}'", t.to_string().to_lowercase()),
        None => "NULL".to_string(),
    };

    let granularity_sql = match granularity {
        Some(g) => format!("'{}'", g),
        None => "NULL".to_string(),
    };

    let desc_sql = match description {
        Some(d) => format!("'{}'", d.replace('\'', "''")),
        None => "NULL".to_string(),
    };

    format!(
        "INSERT INTO data_dictionary.gold_tables \
         (table_name, object_type, source_silver_table, source_stream_type, granularity, description) \
         VALUES ('{table_name}', '{object_type}', {source_silver}, {source_type}, {granularity}, {desc}) \
         ON CONFLICT (table_name) DO UPDATE SET \
         object_type = EXCLUDED.object_type, \
         source_silver_table = EXCLUDED.source_silver_table, \
         source_stream_type = EXCLUDED.source_stream_type, \
         granularity = EXCLUDED.granularity, \
         description = COALESCE(EXCLUDED.description, data_dictionary.gold_tables.description), \
         updated_at = NOW();",
        table_name = table_name,
        object_type = object_type,
        source_silver = source_silver_sql,
        source_type = source_type_sql,
        granularity = granularity_sql,
        desc = desc_sql
    )
}

/// Classification syncer trait for London TDD mocking
pub trait ClassificationSyncer {
    /// Sync classification for a single stream
    fn sync_classification(
        &self,
        stream_id: &str,
        stream_type: StreamType,
        description: Option<&str>,
    ) -> Result<String, GoldDdlError>;

    /// Generate classification SQL without executing
    fn generate_sql(
        &self,
        stream_id: &str,
        stream_type: StreamType,
        description: Option<&str>,
    ) -> String;
}

/// Default implementation of ClassificationSyncer
pub struct DefaultClassificationSyncer;

impl ClassificationSyncer for DefaultClassificationSyncer {
    fn sync_classification(
        &self,
        stream_id: &str,
        stream_type: StreamType,
        description: Option<&str>,
    ) -> Result<String, GoldDdlError> {
        Ok(self.generate_sql(stream_id, stream_type, description))
    }

    fn generate_sql(
        &self,
        stream_id: &str,
        stream_type: StreamType,
        description: Option<&str>,
    ) -> String {
        generate_classification_sql(stream_id, stream_type, description)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== TDD CYCLE 1: Generate Classification SQL ==========

    #[test]
    fn test_generates_classification_insert() {
        // Arrange
        let stream_id = "air-quality";
        let stream_type = StreamType::Observation;

        // Act
        let sql = generate_classification_sql(stream_id, stream_type, None);

        // Assert
        assert!(sql.contains("INSERT INTO data_dictionary.stream_classification"));
        assert!(sql.contains("'air-quality'"));
        assert!(sql.contains("'observation'"));
    }

    #[test]
    fn test_observation_has_effect_role() {
        // Arrange
        let stream_type = StreamType::Observation;

        // Act
        let sql = generate_classification_sql("test", stream_type, None);

        // Assert
        assert!(sql.contains("'effect'"));
    }

    #[test]
    fn test_state_event_has_cause_role() {
        // Arrange
        let stream_type = StreamType::StateEvent;

        // Act
        let sql = generate_classification_sql("test", stream_type, None);

        // Assert
        assert!(sql.contains("'cause'"));
    }

    #[test]
    fn test_forecast_has_context_role() {
        // Arrange
        let stream_type = StreamType::Forecast;

        // Act
        let sql = generate_classification_sql("test", stream_type, None);

        // Assert
        assert!(sql.contains("'context'"));
    }

    #[test]
    fn test_dimension_has_metadata_role() {
        // Arrange
        let stream_type = StreamType::Dimension;

        // Act
        let sql = generate_classification_sql("test", stream_type, None);

        // Assert
        assert!(sql.contains("'metadata'"));
    }

    #[test]
    fn test_observation_has_preserve_null_handling() {
        // Arrange
        let stream_type = StreamType::Observation;

        // Act
        let sql = generate_classification_sql("test", stream_type, None);

        // Assert
        assert!(sql.contains("'preserve'"));
    }

    #[test]
    fn test_state_event_has_carry_forward_null_handling() {
        // Arrange
        let stream_type = StreamType::StateEvent;

        // Act
        let sql = generate_classification_sql("test", stream_type, None);

        // Assert
        assert!(sql.contains("'carry_forward'"));
    }

    #[test]
    fn test_includes_on_conflict_upsert() {
        // Arrange
        let sql = generate_classification_sql("test", StreamType::Observation, None);

        // Assert - idempotent UPSERT pattern
        assert!(sql.contains("ON CONFLICT (stream_id) DO UPDATE SET"));
        assert!(sql.contains("updated_at = NOW()"));
    }

    #[test]
    fn test_description_is_optional() {
        // Arrange - no description
        let sql_no_desc = generate_classification_sql("test", StreamType::Observation, None);

        // Assert
        assert!(sql_no_desc.contains("NULL"));

        // Arrange - with description
        let sql_with_desc = generate_classification_sql(
            "test",
            StreamType::Observation,
            Some("Air quality readings"),
        );

        // Assert
        assert!(sql_with_desc.contains("'Air quality readings'"));
    }

    #[test]
    fn test_description_escapes_single_quotes() {
        // Arrange
        let sql = generate_classification_sql("test", StreamType::Observation, Some("It's a test"));

        // Assert - single quote escaped
        assert!(sql.contains("It''s a test"));
    }

    // ========== TDD CYCLE 2: Gold Table SQL ==========

    #[test]
    fn test_generates_gold_table_insert() {
        // Arrange
        let sql = generate_gold_table_sql(
            "gold.air_quality_hourly",
            "continuous_aggregate",
            Some("silver.air_quality_observations"),
            Some(StreamType::Observation),
            Some("1 hour"),
            Some("Hourly aggregates"),
        );

        // Assert
        assert!(sql.contains("INSERT INTO data_dictionary.gold_tables"));
        assert!(sql.contains("'gold.air_quality_hourly'"));
        assert!(sql.contains("'continuous_aggregate'"));
        assert!(sql.contains("'silver.air_quality_observations'"));
        assert!(sql.contains("'observation'"));
        assert!(sql.contains("'1 hour'"));
    }

    #[test]
    fn test_gold_table_handles_null_values() {
        // Arrange
        let sql = generate_gold_table_sql(
            "gold.test_table",
            "materialized_view",
            None,
            None,
            None,
            None,
        );

        // Assert - NULL values for optional fields
        assert!(sql.contains("NULL"));
        assert!(sql.contains("ON CONFLICT (table_name) DO UPDATE SET"));
    }

    // ========== TDD CYCLE 3: Trait Implementation ==========

    #[test]
    fn test_default_syncer_generates_sql() {
        // Arrange
        let syncer = DefaultClassificationSyncer;

        // Act
        let sql = syncer.generate_sql("air-quality", StreamType::Observation, None);

        // Assert
        assert!(sql.contains("INSERT INTO data_dictionary.stream_classification"));
        assert!(sql.contains("'air-quality'"));
    }

    #[test]
    fn test_default_syncer_sync_returns_sql() {
        // Arrange
        let syncer = DefaultClassificationSyncer;

        // Act
        let result = syncer.sync_classification("air-quality", StreamType::Observation, None);

        // Assert
        assert!(result.is_ok());
        let sql = result.unwrap();
        assert!(sql.contains("INSERT INTO data_dictionary.stream_classification"));
    }
}
