//! Intelligence crate error types

/// Unified error type for the intelligence crate.
#[derive(Debug, thiserror::Error)]
pub enum IntelligenceError {
    /// Storage backend error
    #[error("Storage error: {0}")]
    Storage(#[from] crate::storage::StorageError),

    /// Graph backend error
    #[error("Graph error: {0}")]
    Graph(#[from] crate::graph::GraphError),

    /// Similarity engine error
    #[error("Similarity error: {0}")]
    Similarity(#[from] crate::similarity::SimilarityError),

    /// Embedding error from ndp-lib
    #[error("Embedding error: {0}")]
    Embedding(#[from] ndp_lib::gold::embeddings::EmbeddingError),

    /// Configuration error
    #[error("Configuration error: {message}")]
    Config { message: String },

    /// Raw database error (connection, query failures)
    #[error("Database error: {0}")]
    Database(String),

    /// Graceful shutdown signal received
    #[error("Shutdown signal received")]
    Shutdown,
}

/// Convenience result type for intelligence operations.
pub type Result<T> = std::result::Result<T, IntelligenceError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intelligence_error_display() {
        let err = IntelligenceError::Config {
            message: "missing field".to_string(),
        };
        assert_eq!(err.to_string(), "Configuration error: missing field");
    }

    #[test]
    fn test_database_error_display() {
        let err = IntelligenceError::Database("connection refused".to_string());
        assert_eq!(err.to_string(), "Database error: connection refused");
    }

    #[test]
    fn test_shutdown_error_display() {
        let err = IntelligenceError::Shutdown;
        assert_eq!(err.to_string(), "Shutdown signal received");
    }

    #[test]
    fn test_storage_error_conversion() {
        let storage_err = crate::storage::StorageError::Database("conn failed".to_string());
        let intel_err: IntelligenceError = storage_err.into();
        assert!(matches!(intel_err, IntelligenceError::Storage(_)));
    }

    #[test]
    fn test_graph_error_conversion() {
        let graph_err = crate::graph::GraphError::NodeNotFound {
            id: "n1".to_string(),
        };
        let intel_err: IntelligenceError = graph_err.into();
        assert!(matches!(intel_err, IntelligenceError::Graph(_)));
    }
}
