//! Similarity engine trait and types
//!
//! Defines the `SimilarityEngine` trait for vector similarity search.
//! Implementations: HnswEngine (ruvector, feature-gated), PgVectorEngine (SQL fallback),
//! DualSimilarityEngine (HNSW wrapper, feature-gated).

pub mod dual;
pub mod hnsw;
pub mod pgvector;

use std::sync::Arc;

use deadpool_postgres::Pool;
use tracing::info;

use crate::storage::StorageBackend;
use ndp_lib::gold::embeddings::config::IntelligenceConfig;

/// A vector entry stored in the similarity index.
#[derive(Debug, Clone)]
pub struct VectorEntry {
    /// Unique identifier for this vector
    pub id: String,
    /// The embedding vector
    pub vector: Vec<f32>,
    /// Arbitrary metadata
    pub metadata: serde_json::Value,
}

/// A similarity search query.
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// Query vector
    pub vector: Vec<f32>,
    /// Number of nearest neighbors to return
    pub k: usize,
    /// Minimum similarity threshold (0.0 to 1.0)
    pub min_similarity: f64,
}

/// A single search result with similarity score.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// ID of the matching vector
    pub id: String,
    /// Similarity score (0.0 to 1.0, higher is more similar)
    pub similarity: f64,
    /// Metadata of the matching vector
    pub metadata: serde_json::Value,
}

/// Errors from similarity engine operations.
#[derive(Debug, thiserror::Error)]
pub enum SimilarityError {
    /// Vector dimensions don't match the index dimensions
    #[error("Dimension mismatch: index expects {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// The index has no vectors
    #[error("Index is empty")]
    EmptyIndex,

    /// Backend-specific error
    #[error("Backend error: {0}")]
    Backend(String),
}

/// Trait for vector similarity search engines.
///
/// Phase 1 defines this trait only. Implementations (HNSW via ruvector-core,
/// pgvector) will be added in Phase 2.
pub trait SimilarityEngine: Send + Sync {
    /// Insert a vector entry into the index.
    fn insert(&mut self, entry: VectorEntry) -> std::result::Result<(), SimilarityError>;

    /// Search for the k nearest neighbors of the query vector.
    fn search(
        &self,
        query: &SearchQuery,
    ) -> std::result::Result<Vec<SearchResult>, SimilarityError>;

    /// Return the number of vectors in the index.
    fn count(&self) -> usize;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_entry_construction() {
        let entry = VectorEntry {
            id: "v1".to_string(),
            vector: vec![1.0, 2.0, 3.0],
            metadata: serde_json::json!({"source": "test"}),
        };
        assert_eq!(entry.id, "v1");
        assert_eq!(entry.vector.len(), 3);
    }

    #[test]
    fn test_search_query_construction() {
        let query = SearchQuery {
            vector: vec![1.0, 0.0, 0.0],
            k: 5,
            min_similarity: 0.8,
        };
        assert_eq!(query.k, 5);
        assert!((query.min_similarity - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_search_result_construction() {
        let result = SearchResult {
            id: "v1".to_string(),
            similarity: 0.95,
            metadata: serde_json::json!({}),
        };
        assert!((result.similarity - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn test_similarity_error_display() {
        let err = SimilarityError::DimensionMismatch {
            expected: 10,
            actual: 5,
        };
        assert_eq!(
            err.to_string(),
            "Dimension mismatch: index expects 10, got 5"
        );

        let err = SimilarityError::EmptyIndex;
        assert_eq!(err.to_string(), "Index is empty");

        let err = SimilarityError::Backend("connection lost".to_string());
        assert_eq!(err.to_string(), "Backend error: connection lost");
    }

    #[test]
    fn test_similarity_engine_is_object_safe() {
        // This test verifies the trait can be used as a trait object
        fn _accept_engine(_engine: &dyn SimilarityEngine) {}
    }
}

/// Create the appropriate SimilarityEngine based on feature flags and configuration.
///
/// When the `ruvector` feature is enabled, creates a DualSimilarityEngine that
/// wraps HNSW for fast search, rebuilding the index from stored embeddings.
/// When `ruvector` is not available, falls back to PgVectorEngine for SQL K-NN.
pub async fn create_similarity_engine(
    _config: &IntelligenceConfig,
    storage: Arc<dyn StorageBackend>,
    pool: Arc<Pool>,
    dimensions: usize,
    domain_id: &str,
) -> Result<Box<dyn SimilarityEngine>, SimilarityError> {
    #[cfg(feature = "ruvector")]
    {
        let mut hnsw_engine =
            hnsw::HnswEngine::new(dimensions)?;
        let count = hnsw_engine
            .rebuild_from_storage(storage.as_ref(), domain_id)
            .await?;
        info!(
            "Using DualSimilarityEngine (HNSW with {} vectors)",
            count
        );
        Ok(Box::new(dual::DualSimilarityEngine::new(hnsw_engine)))
    }
    #[cfg(not(feature = "ruvector"))]
    {
        let _ = storage; // unused without ruvector
        info!("Using PgVectorEngine (ruvector feature not enabled)");
        Ok(Box::new(pgvector::PgVectorEngine::new(
            pool,
            dimensions,
            domain_id.to_string(),
        )))
    }
}
