//! Storage backend trait and types
//!
//! Defines the `StorageBackend` trait for embedding and prediction persistence.
//! Phase 1 provides PostgresStorage implementation.

pub mod postgres;

use chrono::{DateTime, Utc};

/// A stored embedding record.
#[derive(Debug, Clone)]
pub struct StoredEmbedding {
    /// Time bucket for this embedding
    pub bucket: DateTime<Utc>,
    /// Domain identifier (e.g., "indoor-air-quality")
    pub domain_id: String,
    /// The embedding vector
    pub embedding: Vec<f32>,
    /// Vector dimensions
    pub dimensions: usize,
    /// Arbitrary metadata
    pub metadata: serde_json::Value,
    /// When this record was created
    pub created_at: DateTime<Utc>,
}

/// A prediction record.
#[derive(Debug, Clone)]
pub struct Prediction {
    /// Database ID (None for new predictions)
    pub id: Option<i64>,
    /// Time bucket
    pub bucket: DateTime<Utc>,
    /// Domain identifier
    pub domain_id: String,
    /// Metric being predicted
    pub metric: String,
    /// Prediction horizon (e.g., "1 hour", "24 hours")
    pub horizon: String,
    /// Predicted value (if applicable)
    pub predicted_value: Option<f64>,
    /// Predicted breach of threshold
    pub predicted_breach: Option<bool>,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Number of neighbors used for prediction
    pub k_neighbors: i32,
    /// Number of neighbors supporting the prediction
    pub k_supporting: i32,
    /// Actual observed value (filled in later)
    pub actual_value: Option<f64>,
    /// Actual breach observed (filled in later)
    pub actual_breach: Option<bool>,
    /// Whether prediction was correct (filled in later)
    pub correct: Option<bool>,
    /// When the outcome was evaluated
    pub evaluated_at: Option<DateTime<Utc>>,
}

/// An actual outcome to record against a prediction.
#[derive(Debug, Clone)]
pub struct ActualOutcome {
    /// The actual observed value
    pub actual_value: f64,
    /// Whether a breach actually occurred
    pub actual_breach: bool,
    /// When this outcome was evaluated
    pub evaluated_at: DateTime<Utc>,
}

/// Errors from storage operations.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// Database-level error
    #[error("Database error: {0}")]
    Database(String),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Record not found
    #[error("Record not found: {entity} with id {id}")]
    NotFound { entity: String, id: String },
}

/// Trait for embedding and prediction storage backends.
#[async_trait::async_trait]
pub trait StorageBackend: Send + Sync {
    /// Store a single embedding.
    async fn store_embedding(
        &self,
        embedding: &StoredEmbedding,
    ) -> std::result::Result<(), StorageError>;

    /// Load embeddings for a domain, optionally filtered by time.
    async fn load_embeddings(
        &self,
        domain_id: &str,
        since: Option<DateTime<Utc>>,
    ) -> std::result::Result<Vec<StoredEmbedding>, StorageError>;

    /// Store a prediction and return its ID.
    async fn store_prediction(
        &self,
        prediction: &Prediction,
    ) -> std::result::Result<i64, StorageError>;

    /// Get predictions awaiting outcomes (actual_value IS NULL).
    async fn get_pending_outcomes(
        &self,
        domain_id: &str,
    ) -> std::result::Result<Vec<Prediction>, StorageError>;

    /// Record an actual outcome against a prediction.
    async fn record_outcome(
        &self,
        prediction_id: i64,
        actual: &ActualOutcome,
    ) -> std::result::Result<(), StorageError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stored_embedding_construction() {
        let now = Utc::now();
        let emb = StoredEmbedding {
            bucket: now,
            domain_id: "test-domain".to_string(),
            embedding: vec![0.1, 0.2, 0.3],
            dimensions: 3,
            metadata: serde_json::json!({}),
            created_at: now,
        };
        assert_eq!(emb.dimensions, 3);
        assert_eq!(emb.embedding.len(), 3);
    }

    #[test]
    fn test_prediction_construction() {
        let pred = Prediction {
            id: None,
            bucket: Utc::now(),
            domain_id: "test".to_string(),
            metric: "pm25".to_string(),
            horizon: "1 hour".to_string(),
            predicted_value: Some(25.0),
            predicted_breach: Some(false),
            confidence: 0.85,
            k_neighbors: 10,
            k_supporting: 8,
            actual_value: None,
            actual_breach: None,
            correct: None,
            evaluated_at: None,
        };
        assert!(pred.id.is_none());
        assert!((pred.confidence - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_actual_outcome_construction() {
        let outcome = ActualOutcome {
            actual_value: 30.0,
            actual_breach: true,
            evaluated_at: Utc::now(),
        };
        assert!((outcome.actual_value - 30.0).abs() < f64::EPSILON);
        assert!(outcome.actual_breach);
    }

    #[test]
    fn test_storage_error_display() {
        let err = StorageError::Database("connection refused".to_string());
        assert_eq!(err.to_string(), "Database error: connection refused");

        let err = StorageError::NotFound {
            entity: "prediction".to_string(),
            id: "42".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Record not found: prediction with id 42"
        );
    }

    #[test]
    fn test_storage_backend_is_object_safe() {
        fn _accept_backend(_backend: &dyn StorageBackend) {}
    }
}
