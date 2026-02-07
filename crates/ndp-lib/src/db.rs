//! Database client abstraction.
//!
//! Provides a trait-based abstraction over database connections for testability.
//! Extracted from ndp-gold-ddl and enhanced with execute/batch_execute methods.

use async_trait::async_trait;
use tokio_postgres::types::ToSql;
use tokio_postgres::{Client, NoTls, Row};

use crate::error::{NdpLibError, Result};

/// Database client trait for mockability.
///
/// All sync operations take `&(impl DbClient + Send + Sync)` so tests
/// can provide mocks without a running database.
#[async_trait]
pub trait DbClient: Send + Sync {
    /// Execute a query and return rows.
    async fn query(&self, query: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>>;

    /// Execute a statement and return the number of rows affected.
    async fn execute(&self, query: &str, params: &[&(dyn ToSql + Sync)]) -> Result<u64>;

    /// Execute raw SQL (multiple statements, no parameters).
    async fn batch_execute(&self, sql: &str) -> Result<()>;
}

/// PostgreSQL client implementation.
pub struct PostgresClient {
    client: Client,
}

impl PostgresClient {
    /// Connect to a PostgreSQL database.
    ///
    /// # Arguments
    /// * `database_url` - PostgreSQL connection string (e.g. `postgresql://user:pass@host:port/db`)
    /// * `timeout_secs` - Connection timeout in seconds
    pub async fn connect(database_url: &str, timeout_secs: u64) -> Result<Self> {
        if !database_url.starts_with("postgresql://") && !database_url.starts_with("postgres://") {
            return Err(NdpLibError::Database(
                "URL must start with postgresql:// or postgres://".to_string(),
            ));
        }

        let connect_future = tokio_postgres::connect(database_url, NoTls);

        let (client, connection) =
            tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), connect_future)
                .await
                .map_err(|_| {
                    NdpLibError::Database(format!("Connection timeout after {}s", timeout_secs))
                })?
                .map_err(|e| NdpLibError::Database(e.to_string()))?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::error!("Database connection error: {}", e);
            }
        });

        Ok(Self { client })
    }
}

#[async_trait]
impl DbClient for PostgresClient {
    async fn query(&self, query: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>> {
        self.client
            .query(query, params)
            .await
            .map_err(|e| NdpLibError::Database(e.to_string()))
    }

    async fn execute(&self, query: &str, params: &[&(dyn ToSql + Sync)]) -> Result<u64> {
        self.client
            .execute(query, params)
            .await
            .map_err(|e| NdpLibError::Database(e.to_string()))
    }

    async fn batch_execute(&self, sql: &str) -> Result<()> {
        self.client
            .batch_execute(sql)
            .await
            .map_err(|e| NdpLibError::Database(e.to_string()))
    }
}

/// No-op database client for dry-run mode.
///
/// All methods return empty results. Used when database operations
/// should be skipped (e.g., `--dry-run` mode in CLI commands).
pub struct NoOpDbClient;

#[async_trait]
impl DbClient for NoOpDbClient {
    async fn query(&self, _query: &str, _params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>> {
        Ok(vec![])
    }

    async fn execute(&self, _query: &str, _params: &[&(dyn ToSql + Sync)]) -> Result<u64> {
        Ok(0)
    }

    async fn batch_execute(&self, _sql: &str) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_url_rejected() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let result = PostgresClient::connect("http://localhost/db", 5).await;
            assert!(matches!(result, Err(NdpLibError::Database(_))));
            if let Err(NdpLibError::Database(msg)) = result {
                assert!(msg.contains("URL must start with"));
            }
        });
    }

    #[test]
    fn test_postgres_url_accepted() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Will fail to connect but should pass URL validation
            let result =
                PostgresClient::connect("postgresql://user:pass@localhost:5432/db", 1).await;
            assert!(result.is_err());
            if let Err(NdpLibError::Database(msg)) = &result {
                assert!(
                    !msg.contains("URL must start with"),
                    "Expected connection error, not URL validation error: {}",
                    msg
                );
            }
        });
    }

    #[test]
    fn test_alternate_postgres_url_accepted() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let result = PostgresClient::connect("postgres://user:pass@localhost:5432/db", 1).await;
            assert!(result.is_err());
            if let Err(NdpLibError::Database(msg)) = &result {
                assert!(
                    !msg.contains("URL must start with"),
                    "Expected connection error, not URL validation error: {}",
                    msg
                );
            }
        });
    }
}
