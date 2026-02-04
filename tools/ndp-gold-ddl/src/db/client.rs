//! Database client abstraction
//!
//! Provides a trait-based abstraction over database connections for testability.

use async_trait::async_trait;
use std::error::Error as StdError;
use thiserror::Error;
use tokio_postgres::{Client, NoTls};

/// Database connection errors
#[derive(Error, Debug)]
pub enum DbError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Query failed: {0}")]
    QueryFailed(String),

    #[error("Invalid database URL: {0}")]
    InvalidUrl(String),

    #[error("Connection timeout after {0} seconds")]
    Timeout(u64),
}

/// Database client trait for mockability
#[async_trait]
pub trait DbClient: Send + Sync {
    /// Execute a query and return rows
    async fn query(
        &self,
        query: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> Result<Vec<tokio_postgres::Row>, DbError>;
}

/// PostgreSQL client implementation
pub struct PostgresClient {
    client: Client,
}

impl PostgresClient {
    /// Connect to PostgreSQL database
    ///
    /// # Arguments
    /// * `database_url` - PostgreSQL connection string (e.g., postgresql://user:pass@host:port/db)
    /// * `timeout_secs` - Connection timeout in seconds
    pub async fn connect(database_url: &str, timeout_secs: u64) -> Result<Self, DbError> {
        // Parse and validate URL
        if !database_url.starts_with("postgresql://") && !database_url.starts_with("postgres://") {
            return Err(DbError::InvalidUrl(
                "URL must start with postgresql:// or postgres://".to_string(),
            ));
        }

        // Connect with timeout
        let connect_future = tokio_postgres::connect(database_url, NoTls);

        let (client, connection) = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            connect_future,
        )
        .await
        .map_err(|_| DbError::Timeout(timeout_secs))?
        .map_err(|e| {
            // Build detailed error message including source chain
            let mut msg = e.to_string();
            let mut source = e.source();
            while let Some(s) = source {
                msg.push_str(&format!(" (caused by: {})", s));
                source = s.source();
            }
            DbError::ConnectionFailed(msg)
        })?;

        // Spawn connection handler
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
    async fn query(
        &self,
        query: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> Result<Vec<tokio_postgres::Row>, DbError> {
        self.client
            .query(query, params)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))
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
            assert!(matches!(result, Err(DbError::InvalidUrl(_))));
        });
    }

    #[test]
    fn test_postgres_url_accepted() {
        // This will fail to connect but should pass URL validation
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let result = PostgresClient::connect("postgresql://user:pass@localhost:5432/db", 1).await;
            // Should be connection error or timeout, not InvalidUrl
            assert!(!matches!(result, Err(DbError::InvalidUrl(_))));
        });
    }

    #[test]
    fn test_alternate_postgres_url_accepted() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let result = PostgresClient::connect("postgres://user:pass@localhost:5432/db", 1).await;
            assert!(!matches!(result, Err(DbError::InvalidUrl(_))));
        });
    }
}
