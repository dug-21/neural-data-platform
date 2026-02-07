//! Database queries for continuous aggregate checks
//!
//! Provides trait-based abstraction for checking CA existence in TimescaleDB.
//! Uses `crate::DbClient` (ndp-lib's shared trait) for database access.

use async_trait::async_trait;

use crate::gold::error::GoldDdlError;
use crate::DbClient;

/// Information about an existing continuous aggregate
#[derive(Debug, Clone, PartialEq)]
pub struct CaInfo {
    pub schema: String,
    pub name: String,
    pub view_definition: Option<String>,
}

/// Trait for checking continuous aggregate existence
#[async_trait]
pub trait CaChecker: Send + Sync {
    /// Check if a continuous aggregate exists
    async fn ca_exists(&self, schema: &str, name: &str) -> Result<bool, GoldDdlError>;

    /// Get info about a continuous aggregate if it exists
    async fn get_ca_info(&self, schema: &str, name: &str) -> Result<Option<CaInfo>, GoldDdlError>;

    /// List all continuous aggregates in a schema
    async fn list_cas_in_schema(&self, schema: &str) -> Result<Vec<CaInfo>, GoldDdlError>;

    /// Check if a refresh policy exists for a continuous aggregate
    async fn refresh_policy_exists(&self, schema: &str, name: &str) -> Result<bool, GoldDdlError>;
}

/// PostgreSQL/TimescaleDB implementation of CaChecker.
/// Uses `crate::DbClient` for database access.
pub struct PostgresCaChecker<C: DbClient> {
    client: C,
}

impl<C: DbClient> PostgresCaChecker<C> {
    pub fn new(client: C) -> Self {
        Self { client }
    }
}

#[async_trait]
impl<C: DbClient + Send + Sync> CaChecker for PostgresCaChecker<C> {
    async fn ca_exists(&self, schema: &str, name: &str) -> Result<bool, GoldDdlError> {
        let query = r#"
            SELECT EXISTS (
                SELECT 1
                FROM timescaledb_information.continuous_aggregates
                WHERE view_schema = $1 AND view_name = $2
            ) AS exists
        "#;

        let rows = self
            .client
            .query(query, &[&schema, &name])
            .await
            .map_err(|e| GoldDdlError::DatabaseError(e.to_string()))?;

        Ok(rows
            .first()
            .map(|r| r.get::<_, bool>("exists"))
            .unwrap_or(false))
    }

    async fn get_ca_info(&self, schema: &str, name: &str) -> Result<Option<CaInfo>, GoldDdlError> {
        let query = r#"
            SELECT
                view_schema,
                view_name,
                view_definition
            FROM timescaledb_information.continuous_aggregates
            WHERE view_schema = $1 AND view_name = $2
        "#;

        let rows = self
            .client
            .query(query, &[&schema, &name])
            .await
            .map_err(|e| GoldDdlError::DatabaseError(e.to_string()))?;

        Ok(rows.first().map(|row| CaInfo {
            schema: row.get("view_schema"),
            name: row.get("view_name"),
            view_definition: row.get("view_definition"),
        }))
    }

    async fn list_cas_in_schema(&self, schema: &str) -> Result<Vec<CaInfo>, GoldDdlError> {
        let query = r#"
            SELECT
                view_schema,
                view_name,
                view_definition
            FROM timescaledb_information.continuous_aggregates
            WHERE view_schema = $1
            ORDER BY view_name
        "#;

        let rows = self
            .client
            .query(query, &[&schema])
            .await
            .map_err(|e| GoldDdlError::DatabaseError(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|row| CaInfo {
                schema: row.get("view_schema"),
                name: row.get("view_name"),
                view_definition: row.get("view_definition"),
            })
            .collect())
    }

    async fn refresh_policy_exists(&self, schema: &str, name: &str) -> Result<bool, GoldDdlError> {
        let query = r#"
            SELECT EXISTS (
                SELECT 1
                FROM timescaledb_information.jobs j
                JOIN timescaledb_information.continuous_aggregates ca
                    ON j.hypertable_schema = ca.materialization_hypertable_schema
                    AND j.hypertable_name = ca.materialization_hypertable_name
                WHERE ca.view_schema = $1
                    AND ca.view_name = $2
                    AND j.proc_name = 'policy_refresh_continuous_aggregate'
            ) AS exists
        "#;

        let rows = self
            .client
            .query(query, &[&schema, &name])
            .await
            .map_err(|e| GoldDdlError::DatabaseError(e.to_string()))?;

        Ok(rows
            .first()
            .map(|r| r.get::<_, bool>("exists"))
            .unwrap_or(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full mock tests for PostgresCaChecker would require creating mock Row objects,
    // which is complex with tokio-postgres. The planner module has comprehensive tests
    // using a simple MockCaChecker. Integration tests with real DB provide full coverage
    // for the PostgresCaChecker implementation.

    #[test]
    fn test_ca_info_equality() {
        let info1 = CaInfo {
            schema: "gold".to_string(),
            name: "air_quality_hourly".to_string(),
            view_definition: None,
        };

        let info2 = CaInfo {
            schema: "gold".to_string(),
            name: "air_quality_hourly".to_string(),
            view_definition: None,
        };

        assert_eq!(info1, info2);
    }

    #[test]
    fn test_ca_info_clone() {
        let info = CaInfo {
            schema: "gold".to_string(),
            name: "test_ca".to_string(),
            view_definition: Some("SELECT ...".to_string()),
        };

        let cloned = info.clone();
        assert_eq!(info, cloned);
    }

    #[test]
    fn test_ca_info_debug() {
        let info = CaInfo {
            schema: "gold".to_string(),
            name: "test_ca".to_string(),
            view_definition: None,
        };

        let debug_str = format!("{:?}", info);
        assert!(debug_str.contains("gold"));
        assert!(debug_str.contains("test_ca"));
    }
}
