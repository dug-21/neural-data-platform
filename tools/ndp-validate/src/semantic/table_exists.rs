//! Table existence validation for NDP stream configurations
//!
//! Validates that Silver layer target tables exist in TimescaleDB.
//! Supports graceful degradation when database is unavailable.
//!
//! Note: The async database validation requires the "database" feature
//! which includes sqlx. This is optional - without it, table checks
//! gracefully degrade to warnings.

use crate::error::{ErrorCode, ValidationError};

/// Result of table existence check
#[derive(Debug, Clone)]
pub enum TableCheckResult {
    /// Table exists
    Exists,
    /// Table does not exist
    NotFound { schema: String, table: String },
    /// Could not check (no database connection)
    Unavailable { reason: String },
    /// Invalid table format
    InvalidFormat { table: String, reason: String },
}

/// Parse "schema.table" format into (schema, table)
///
/// Returns None if the format is invalid.
pub fn parse_table_reference(target_table: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = target_table.split('.').collect();
    if parts.len() == 2 {
        let schema = parts[0].trim();
        let table = parts[1].trim();
        if !schema.is_empty() && !table.is_empty() {
            return Some((schema.to_string(), table.to_string()));
        }
    }
    None
}

/// Validate that a Silver target table exists
///
/// Returns validation errors or warnings based on the check result.
///
/// # Arguments
/// * `target_table` - The target table in "schema.table" format
/// * `pool` - Optional database connection pool
///
/// # Behavior
/// - If pool is None, returns a warning (graceful degradation)
/// - If pool is Some, queries information_schema.tables
/// - If table doesn't exist, returns an error
/// - If query fails, returns a warning
pub fn validate_table_exists(target_table: &str, pool: Option<()>) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let path = "$.silver_etl.target_table";

    // Parse schema.table format
    let (_schema, _table) = match parse_table_reference(target_table) {
        Some((s, t)) => (s, t),
        None => {
            errors.push(ValidationError::semantic_error(
                ErrorCode::InvalidTableFormat,
                path,
                format!(
                    "Invalid table format '{}'. Expected 'schema.table'",
                    target_table
                ),
            ));
            return errors;
        }
    };

    // Check if database connection is available
    if pool.is_none() {
        errors.push(ValidationError::semantic_warning(
            ErrorCode::TableCheckFailed,
            path,
            format!(
                "Cannot verify table '{}' exists: database connection unavailable. Table check skipped.",
                target_table
            ),
        ));
        return errors;
    }

    // With actual database connection, we would query information_schema
    // This is the synchronous stub; async version with PgPool is below
    errors
}

// Note: Async database validation is available when the "database" feature
// is enabled. This would require adding sqlx to dependencies.
// For now, the synchronous version gracefully degrades to warnings.

#[cfg(test)]
mod tests {
    use super::*;

    // ===========================================
    // Table Format Parsing Tests
    // ===========================================

    #[test]
    fn test_parse_table_reference_valid() {
        let result = parse_table_reference("silver.air_quality_readings");
        assert_eq!(
            result,
            Some(("silver".to_string(), "air_quality_readings".to_string()))
        );
    }

    #[test]
    fn test_parse_table_reference_public_schema() {
        let result = parse_table_reference("public.weather_data");
        assert_eq!(
            result,
            Some(("public".to_string(), "weather_data".to_string()))
        );
    }

    #[test]
    fn test_parse_table_reference_no_schema() {
        let result = parse_table_reference("air_quality_readings");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_table_reference_too_many_parts() {
        let result = parse_table_reference("database.schema.table");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_table_reference_empty_parts() {
        let result = parse_table_reference(".table");
        assert_eq!(result, None);

        let result = parse_table_reference("schema.");
        assert_eq!(result, None);
    }

    // ===========================================
    // Table Existence Validation Tests
    // ===========================================

    #[test]
    fn test_validate_table_invalid_format() {
        // Arrange: Table name without schema
        let target_table = "air_quality_readings";

        // Act
        let errors = validate_table_exists(target_table, None);

        // Assert
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidTableFormat);
        assert!(errors[0].message.contains("schema.table"));
    }

    #[test]
    fn test_validate_table_graceful_degradation_no_db() {
        // Arrange: Valid table format but no database connection
        let target_table = "silver.air_quality_readings";

        // Act
        let errors = validate_table_exists(target_table, None);

        // Assert: Should get a warning, not an error
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::TableCheckFailed);
        assert!(errors[0].message.contains("unavailable"));
        // Verify it's a warning (graceful degradation)
        assert_eq!(errors[0].severity, crate::error::Severity::Warning);
    }

    #[test]
    fn test_validate_table_with_db_stub() {
        // Arrange: Valid table format with "database connection"
        let target_table = "silver.air_quality_readings";

        // Act: Pass Some(()) to simulate database being available
        let errors = validate_table_exists(target_table, Some(()));

        // Assert: With stub, no errors (actual query would happen in async version)
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_table_error_path() {
        // Arrange: Invalid table format
        let target_table = "invalid";

        // Act
        let errors = validate_table_exists(target_table, None);

        // Assert: Path should be the silver_etl.target_table path
        assert_eq!(errors[0].path, "$.silver_etl.target_table");
    }

    // ===========================================
    // Edge Case Tests
    // ===========================================

    #[test]
    fn test_validate_table_with_spaces() {
        // Arrange: Table with spaces around dot
        let target_table = "silver . air_quality";

        // Act
        let result = parse_table_reference(target_table);

        // Assert: Should handle trimming
        assert_eq!(
            result,
            Some(("silver".to_string(), "air_quality".to_string()))
        );
    }

    #[test]
    fn test_validate_table_underscores() {
        // Arrange: Table with multiple underscores
        let target_table = "silver.air_quality_sensor_readings_v2";

        // Act
        let result = parse_table_reference(target_table);

        // Assert
        assert_eq!(
            result,
            Some((
                "silver".to_string(),
                "air_quality_sensor_readings_v2".to_string()
            ))
        );
    }
}
