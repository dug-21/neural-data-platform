//! SQL generation helpers for data dictionary sync.
//!
//! Contains type mapping and query-building utilities used by the sync logic.

/// Map a config field type name to a PostgreSQL type string.
///
/// This matches the Bash `case` statement in `sync_to_data_dictionary()`.
///
/// # Examples
/// ```
/// use ndp_lib::dictionary::sql::map_field_type_to_pg;
/// assert_eq!(map_field_type_to_pg("double_precision"), "DOUBLE PRECISION");
/// assert_eq!(map_field_type_to_pg("smallint"), "SMALLINT");
/// assert_eq!(map_field_type_to_pg("unknown"), "TEXT");
/// ```
pub fn map_field_type_to_pg(field_type: &str) -> &'static str {
    match field_type {
        "double_precision" => "DOUBLE PRECISION",
        "smallint" => "SMALLINT",
        "integer" | "int" => "INTEGER",
        "bigint" => "BIGINT",
        "text" => "TEXT",
        "timestamptz" => "TIMESTAMPTZ",
        "boolean" | "bool" => "BOOLEAN",
        "jsonb" => "JSONB",
        _ => "TEXT",
    }
}

/// Extract the schema name from a fully-qualified table name.
///
/// E.g. `"silver.weather_observations"` -> `"silver"`.
/// If there is no dot, returns the whole string (it is its own schema).
pub fn extract_schema_name(table_name: &str) -> &str {
    table_name.split('.').next().unwrap_or("silver")
}

// --------------------------------------------------------------------------
// SQL statement constants
// --------------------------------------------------------------------------

/// INSERT sync_status (running).
pub const INSERT_SYNC_STATUS: &str =
    "INSERT INTO data_dictionary.sync_status (sync_type, status) VALUES ($1, $2)";

/// DELETE all from entity_schema_attributes.
pub const DELETE_ENTITY_SCHEMA_ATTRIBUTES: &str =
    "DELETE FROM data_dictionary.entity_schema_attributes";

/// DELETE all from entity_schemas.
pub const DELETE_ENTITY_SCHEMAS: &str = "DELETE FROM data_dictionary.entity_schemas";

/// DELETE all from sources.
pub const DELETE_SOURCES: &str = "DELETE FROM data_dictionary.sources";

/// DELETE all from fields.
pub const DELETE_FIELDS: &str = "DELETE FROM data_dictionary.fields";

/// DELETE all from streams.
pub const DELETE_STREAMS: &str = "DELETE FROM data_dictionary.streams";

/// INSERT a stream.
pub const INSERT_STREAM: &str = "\
INSERT INTO data_dictionary.streams (stream_id, description, version, enabled, retention_days) \
VALUES ($1, $2, $3, $4, $5)";

/// INSERT a field.
pub const INSERT_FIELD: &str = "\
INSERT INTO data_dictionary.fields \
(stream_id, field_name, field_type, nullable, unit, description, validation_min, validation_max, sort_order) \
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)";

/// INSERT a source.
pub const INSERT_SOURCE: &str = "\
INSERT INTO data_dictionary.sources (stream_id, source_id, source_type, enabled, config, parser_type) \
VALUES ($1, $2, $3, $4, $5, $6)";

/// INSERT an entity schema.
pub const INSERT_ENTITY_SCHEMA: &str = "\
INSERT INTO data_dictionary.entity_schemas (stream_id, schema_name, description, device_class) \
VALUES ($1, $2, $3, $4)";

/// INSERT an entity schema attribute using a subselect for schema_id.
pub const INSERT_ENTITY_SCHEMA_ATTRIBUTE: &str = "\
INSERT INTO data_dictionary.entity_schema_attributes \
(schema_id, attribute_name, attribute_type, unit, description, nullable, sort_order) \
SELECT id, $1, $2, $3, $4, $5, $6 \
FROM data_dictionary.entity_schemas WHERE stream_id = $7 AND schema_name = $8";

/// UPSERT a silver table.
pub const UPSERT_SILVER_TABLE: &str = "\
INSERT INTO data_dictionary.silver_tables \
(table_name, schema_name, description, grain, source_streams, hypertable_column) \
VALUES ($1, $2, $3, $4, $5, $6) \
ON CONFLICT (table_name) DO UPDATE SET \
description = EXCLUDED.description, \
grain = EXCLUDED.grain, \
source_streams = EXCLUDED.source_streams, \
hypertable_column = EXCLUDED.hypertable_column, \
updated_at = NOW()";

/// UPSERT a silver column.
pub const UPSERT_SILVER_COLUMN: &str = "\
INSERT INTO data_dictionary.silver_columns \
(table_name, column_name, data_type, unit, description, nullable, sort_order) \
VALUES ($1, $2, $3, $4, $5, $6, $7) \
ON CONFLICT (table_name, column_name) DO UPDATE SET \
data_type = EXCLUDED.data_type, \
unit = EXCLUDED.unit, \
description = EXCLUDED.description, \
nullable = EXCLUDED.nullable, \
sort_order = EXCLUDED.sort_order, \
updated_at = NOW()";

/// UPSERT a silver lineage record.
pub const UPSERT_SILVER_LINEAGE: &str = "\
INSERT INTO data_dictionary.silver_lineage \
(silver_table, silver_column, source_stream, source_path, transformation) \
VALUES ($1, $2, $3, $4, $5) \
ON CONFLICT (silver_table, silver_column, source_stream) DO UPDATE SET \
source_path = EXCLUDED.source_path, \
transformation = EXCLUDED.transformation, \
updated_at = NOW()";

/// UPSERT a column-level silver DQ rule.
pub const UPSERT_SILVER_DQ_RULE_COLUMN: &str = "\
INSERT INTO data_dictionary.silver_dq_rules \
(silver_table, silver_column, rule_name, rule_params, action) \
VALUES ($1, $2, $3, $4, $5) \
ON CONFLICT (silver_table, COALESCE(silver_column, ''), rule_name) DO UPDATE SET \
rule_params = EXCLUDED.rule_params, \
action = EXCLUDED.action, \
updated_at = NOW()";

/// UPSERT a table-level silver DQ rule (silver_column is NULL).
pub const UPSERT_SILVER_DQ_RULE_TABLE: &str = "\
INSERT INTO data_dictionary.silver_dq_rules \
(silver_table, silver_column, rule_name, rule_params, action) \
VALUES ($1, NULL, $2, $3, $4) \
ON CONFLICT (silver_table, COALESCE(silver_column, ''), rule_name) DO UPDATE SET \
rule_params = EXCLUDED.rule_params, \
action = EXCLUDED.action, \
updated_at = NOW()";

/// UPDATE sync_status to success with counts.
pub const UPDATE_SYNC_STATUS_SUCCESS: &str = "\
UPDATE data_dictionary.sync_status \
SET completed_at = NOW(), \
    status = 'success', \
    streams_synced = $1, \
    schemas_synced = $2, \
    attributes_synced = $3, \
    silver_tables_synced = $4, \
    silver_columns_synced = $5 \
WHERE status = 'running' AND completed_at IS NULL";

/// UPDATE sync_status to failed.
pub const UPDATE_SYNC_STATUS_FAILED: &str = "\
UPDATE data_dictionary.sync_status \
SET completed_at = NOW(), \
    status = 'failed', \
    error_message = $1 \
WHERE status = 'running' AND completed_at IS NULL";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_double_precision() {
        assert_eq!(map_field_type_to_pg("double_precision"), "DOUBLE PRECISION");
    }

    #[test]
    fn test_map_smallint() {
        assert_eq!(map_field_type_to_pg("smallint"), "SMALLINT");
    }

    #[test]
    fn test_map_integer() {
        assert_eq!(map_field_type_to_pg("integer"), "INTEGER");
    }

    #[test]
    fn test_map_int_alias() {
        assert_eq!(map_field_type_to_pg("int"), "INTEGER");
    }

    #[test]
    fn test_map_bigint() {
        assert_eq!(map_field_type_to_pg("bigint"), "BIGINT");
    }

    #[test]
    fn test_map_text() {
        assert_eq!(map_field_type_to_pg("text"), "TEXT");
    }

    #[test]
    fn test_map_timestamptz() {
        assert_eq!(map_field_type_to_pg("timestamptz"), "TIMESTAMPTZ");
    }

    #[test]
    fn test_map_boolean() {
        assert_eq!(map_field_type_to_pg("boolean"), "BOOLEAN");
    }

    #[test]
    fn test_map_bool_alias() {
        assert_eq!(map_field_type_to_pg("bool"), "BOOLEAN");
    }

    #[test]
    fn test_map_jsonb() {
        assert_eq!(map_field_type_to_pg("jsonb"), "JSONB");
    }

    #[test]
    fn test_map_unknown_defaults_to_text() {
        assert_eq!(map_field_type_to_pg("varchar"), "TEXT");
        assert_eq!(map_field_type_to_pg(""), "TEXT");
        assert_eq!(map_field_type_to_pg("float"), "TEXT");
    }

    #[test]
    fn test_extract_schema_name() {
        assert_eq!(extract_schema_name("silver.weather_observations"), "silver");
        assert_eq!(extract_schema_name("public.my_table"), "public");
    }

    #[test]
    fn test_extract_schema_name_no_dot() {
        assert_eq!(extract_schema_name("my_table"), "my_table");
    }
}
