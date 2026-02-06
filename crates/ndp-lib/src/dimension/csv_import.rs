//! CSV parsing for dimension imports.
//!
//! Parses raw CSV content, validates that the header columns match the
//! dimension schema, and returns rows as `Vec<Vec<Option<String>>>`.

use crate::config::DimensionConfig;
use crate::error::{NdpLibError, Result};

/// Parse CSV content into rows matching the dimension schema.
///
/// The returned rows are in field-order as defined in `config.schema.fields`.
/// Each cell is `None` when the CSV value is empty (representing SQL NULL).
///
/// # Errors
///
/// Returns `NdpLibError::Csv` if the header is missing required columns
/// or the CSV is malformed.
pub fn parse_csv(csv_content: &[u8], config: &DimensionConfig) -> Result<Vec<Vec<Option<String>>>> {
    let has_header = config.source.has_header;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(has_header)
        .flexible(true)
        .from_reader(csv_content);

    // Build expected column names from schema
    let expected_columns: Vec<&str> = config
        .schema
        .fields
        .iter()
        .map(|f| f.name.as_str())
        .collect();

    // Validate header columns and build index mapping
    let headers = reader.headers()?.clone();
    let header_vec: Vec<&str> = headers.iter().collect();

    let mut column_map: Vec<usize> = Vec::with_capacity(expected_columns.len());
    for col_name in &expected_columns {
        match header_vec.iter().position(|h| h == col_name) {
            Some(idx) => column_map.push(idx),
            None => {
                return Err(NdpLibError::Csv(format!(
                    "Required column '{}' not found in CSV header. \
                     CSV has: [{}], schema expects: [{}]",
                    col_name,
                    header_vec.join(", "),
                    expected_columns.join(", "),
                )));
            }
        }
    }

    // Parse rows
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for result in reader.records() {
        let record = result?;
        let mut row: Vec<Option<String>> = Vec::with_capacity(expected_columns.len());
        for &csv_idx in &column_map {
            let value = record.get(csv_idx).unwrap_or("");
            if value.is_empty() {
                row.push(None);
            } else {
                row.push(Some(value.to_string()));
            }
        }
        rows.push(row);
    }

    Ok(rows)
}

/// Build a parameterized INSERT SQL statement for a batch of rows.
///
/// Returns `(sql, params_per_row)` where `sql` contains `$1`, `$2`, etc.
/// placeholders. Each row occupies `params_per_row` parameter slots.
///
/// Type handling is done at the parameter level (see `sync_dimension`),
/// not in the SQL. Array columns receive `Vec<String>` params, text
/// columns receive `Option<String>`.
pub fn build_insert_sql(config: &DimensionConfig, row_count: usize) -> (String, usize) {
    let schema_name = &config.target.schema;
    let table_name = &config.target.table;
    let columns: Vec<&str> = config
        .schema
        .fields
        .iter()
        .map(|f| f.name.as_str())
        .collect();
    let params_per_row = columns.len();

    let column_list = columns.join(", ");

    let mut value_groups: Vec<String> = Vec::with_capacity(row_count);
    for row_idx in 0..row_count {
        let base = row_idx * params_per_row;
        let placeholders: Vec<String> = (1..=params_per_row)
            .map(|i| format!("${}", base + i))
            .collect();
        value_groups.push(format!("({})", placeholders.join(", ")));
    }

    let sql = format!(
        "INSERT INTO {}.{} ({}) VALUES {}",
        schema_name,
        table_name,
        column_list,
        value_groups.join(", "),
    );

    (sql, params_per_row)
}

/// Parse a PostgreSQL array literal string into a Vec of strings.
///
/// Handles formats like `{a,b,c}` and `{a}`. Returns an empty vec for `{}`.
/// Values are trimmed of whitespace.
pub fn parse_pg_array(s: &str) -> Vec<String> {
    let trimmed = s.trim();
    // Strip outer braces
    let inner = if trimmed.starts_with('{') && trimmed.ends_with('}') {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };
    if inner.is_empty() {
        return Vec::new();
    }
    inner.split(',').map(|v| v.trim().to_string()).collect()
}

/// Build TRUNCATE SQL for the target table.
pub fn build_truncate_sql(config: &DimensionConfig) -> String {
    format!(
        "TRUNCATE TABLE {}.{}",
        config.target.schema, config.target.table,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        DimensionConfig, DimensionField, DimensionLoad, DimensionSchema, DimensionSource,
        DimensionTarget,
    };

    /// Helper to build a minimal test config with the given field names.
    fn test_config(field_names: &[&str]) -> DimensionConfig {
        DimensionConfig {
            dimension_id: "test_dim".to_string(),
            description: "Test".to_string(),
            version: "1.0.0".to_string(),
            target: DimensionTarget {
                table: "test_table".to_string(),
                schema: "silver".to_string(),
            },
            source: DimensionSource {
                source_type: "csv".to_string(),
                path: None,
                delimiter: ",".to_string(),
                has_header: true,
            },
            schema: DimensionSchema {
                primary_key: vec![field_names[0].to_string()],
                fields: field_names
                    .iter()
                    .map(|name| DimensionField {
                        name: name.to_string(),
                        field_type: "text".to_string(),
                        nullable: false,
                        description: None,
                    })
                    .collect(),
            },
            load: Some(DimensionLoad {
                strategy: "truncate_and_load".to_string(),
                batch_size: 1000,
            }),
        }
    }

    #[test]
    fn test_parse_csv_basic() {
        let config = test_config(&["id", "name", "value"]);
        let csv = b"id,name,value\n1,alpha,100\n2,beta,200\n";

        let rows = parse_csv(csv, &config).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], vec![s("1"), s("alpha"), s("100")]);
        assert_eq!(rows[1], vec![s("2"), s("beta"), s("200")]);
    }

    #[test]
    fn test_parse_csv_with_empty_fields() {
        let config = test_config(&["id", "name", "value"]);
        let csv = b"id,name,value\n1,,100\n2,beta,\n";

        let rows = parse_csv(csv, &config).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], vec![s("1"), None, s("100")]);
        assert_eq!(rows[1], vec![s("2"), s("beta"), None]);
    }

    #[test]
    fn test_parse_csv_column_validation() {
        let config = test_config(&["id", "name", "missing_col"]);
        let csv = b"id,name,value\n1,alpha,100\n";

        let result = parse_csv(csv, &config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("missing_col"),
            "Error should mention missing column: {}",
            msg
        );
    }

    #[test]
    fn test_parse_csv_reorders_columns() {
        // CSV columns in different order than schema
        let config = test_config(&["id", "name", "value"]);
        let csv = b"value,id,name\n100,1,alpha\n200,2,beta\n";

        let rows = parse_csv(csv, &config).unwrap();
        // Should reorder to match schema: id, name, value
        assert_eq!(rows[0], vec![s("1"), s("alpha"), s("100")]);
        assert_eq!(rows[1], vec![s("2"), s("beta"), s("200")]);
    }

    #[test]
    fn test_build_insert_sql_single_row() {
        let config = test_config(&["id", "name"]);
        let (sql, ppr) = build_insert_sql(&config, 1);
        assert_eq!(ppr, 2);
        assert_eq!(
            sql,
            "INSERT INTO silver.test_table (id, name) VALUES ($1, $2)"
        );
    }

    #[test]
    fn test_build_insert_sql_multiple_rows() {
        let config = test_config(&["id", "name"]);
        let (sql, ppr) = build_insert_sql(&config, 3);
        assert_eq!(ppr, 2);
        assert_eq!(
            sql,
            "INSERT INTO silver.test_table (id, name) VALUES ($1, $2), ($3, $4), ($5, $6)"
        );
    }

    #[test]
    fn test_build_truncate_sql() {
        let config = test_config(&["id"]);
        let sql = build_truncate_sql(&config);
        assert_eq!(sql, "TRUNCATE TABLE silver.test_table");
    }

    #[test]
    fn test_parse_pg_array_multi() {
        assert_eq!(
            parse_pg_array("{humidity_living,temp_outdoor}"),
            vec!["humidity_living", "temp_outdoor"]
        );
    }

    #[test]
    fn test_parse_pg_array_single() {
        assert_eq!(parse_pg_array("{humidity_living}"), vec!["humidity_living"]);
    }

    #[test]
    fn test_parse_pg_array_empty() {
        let result: Vec<String> = Vec::new();
        assert_eq!(parse_pg_array("{}"), result);
    }

    #[test]
    fn test_parse_pg_array_with_spaces() {
        assert_eq!(
            parse_pg_array("{ a , b , c }"),
            vec!["a", "b", "c"]
        );
    }

    /// Helper: wrap a string into `Some(String)`.
    fn s(v: &str) -> Option<String> {
        Some(v.to_string())
    }
}
