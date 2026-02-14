//! Gold layer embedding types and traits
//!
//! This module provides the foundational types for the intelligence layer:
//! - [`Embedder`] trait for converting Gold rows to vector embeddings
//! - [`GoldRow`] representing a time-bucketed Gold layer record
//! - [`Embedding`] representing a computed vector embedding
//! - [`MetricEmbedder`] implementing metric-based embedding generation
//! - [`RunningStats`] for online mean/std computation with exponential decay
//! - Configuration types for intelligence settings

pub mod config;
pub mod metric;
pub mod stats;

use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, HashMap};

/// Errors from embedding operations.
#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    /// A required field was not found in the GoldRow
    #[error("Field '{field}' not found in GoldRow")]
    FieldNotFound { field: String },

    /// Not enough data to generate a meaningful embedding
    #[error("Insufficient data for embedding: {reason}")]
    InsufficientData { reason: String },

    /// Vector dimensions don't match expected dimensions
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
}

/// Convenience result type for embedding operations.
pub type EmbeddingResult<T> = std::result::Result<T, EmbeddingError>;

/// A time-bucketed record from the Gold layer.
///
/// Uses `BTreeMap` for deterministic field ordering, which ensures
/// consistent embedding vector construction across runs.
#[derive(Debug, Clone)]
pub struct GoldRow {
    /// Time bucket for this record
    pub bucket: DateTime<Utc>,
    /// Domain identifier (e.g., "indoor-air-quality")
    pub domain_id: String,
    /// Field values, ordered deterministically by key.
    /// `None` represents a missing/null value.
    pub fields: BTreeMap<String, Option<f64>>,
}

/// A computed vector embedding.
#[derive(Debug, Clone)]
pub struct Embedding {
    /// The embedding vector (f32 for memory efficiency)
    pub vector: Vec<f32>,
    /// Number of dimensions
    pub dimensions: usize,
    /// Arbitrary metadata about this embedding
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Embedding {
    /// Create a new Embedding, verifying dimensions match vector length.
    pub fn new(
        vector: Vec<f32>,
        metadata: HashMap<String, serde_json::Value>,
    ) -> EmbeddingResult<Self> {
        let dimensions = vector.len();
        Ok(Self {
            vector,
            dimensions,
            metadata,
        })
    }

    /// Create an Embedding with explicit dimension check.
    pub fn with_dimensions(
        vector: Vec<f32>,
        expected_dimensions: usize,
        metadata: HashMap<String, serde_json::Value>,
    ) -> EmbeddingResult<Self> {
        if vector.len() != expected_dimensions {
            return Err(EmbeddingError::DimensionMismatch {
                expected: expected_dimensions,
                actual: vector.len(),
            });
        }
        Ok(Self {
            vector,
            dimensions: expected_dimensions,
            metadata,
        })
    }
}

/// Trait for converting Gold rows into vector embeddings.
///
/// Implementations must be thread-safe (`Send + Sync`).
pub trait Embedder: Send + Sync {
    /// Convert a Gold row into a vector embedding.
    fn embed(&self, row: &GoldRow) -> EmbeddingResult<Embedding>;

    /// Return the number of dimensions this embedder produces.
    fn dimensions(&self) -> usize;

    /// Return the name of this embedder.
    fn name(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gold_row_btreemap_ordering() {
        let mut fields = BTreeMap::new();
        fields.insert("z_field".to_string(), Some(3.0));
        fields.insert("a_field".to_string(), Some(1.0));
        fields.insert("m_field".to_string(), Some(2.0));

        let row = GoldRow {
            bucket: Utc::now(),
            domain_id: "test".to_string(),
            fields,
        };

        // BTreeMap maintains sorted order
        let keys: Vec<&String> = row.fields.keys().collect();
        assert_eq!(keys, vec!["a_field", "m_field", "z_field"]);
    }

    #[test]
    fn test_gold_row_mixed_some_none() {
        let mut fields = BTreeMap::new();
        fields.insert("present".to_string(), Some(42.0));
        fields.insert("missing".to_string(), None);

        let row = GoldRow {
            bucket: Utc::now(),
            domain_id: "test".to_string(),
            fields,
        };

        assert_eq!(row.fields.get("present"), Some(&Some(42.0)));
        assert_eq!(row.fields.get("missing"), Some(&None));
        assert_eq!(row.fields.get("nonexistent"), None);
    }

    #[test]
    fn test_gold_row_deterministic_iteration() {
        let mut fields1 = BTreeMap::new();
        fields1.insert("b".to_string(), Some(2.0));
        fields1.insert("a".to_string(), Some(1.0));
        fields1.insert("c".to_string(), Some(3.0));

        let mut fields2 = BTreeMap::new();
        fields2.insert("c".to_string(), Some(3.0));
        fields2.insert("a".to_string(), Some(1.0));
        fields2.insert("b".to_string(), Some(2.0));

        // Regardless of insertion order, iteration order is the same
        let vals1: Vec<_> = fields1.values().collect();
        let vals2: Vec<_> = fields2.values().collect();
        assert_eq!(vals1, vals2);
    }

    #[test]
    fn test_embedding_new_sets_dimensions() {
        let emb = Embedding::new(vec![1.0, 2.0, 3.0], HashMap::new()).unwrap();
        assert_eq!(emb.dimensions, 3);
        assert_eq!(emb.vector.len(), 3);
    }

    #[test]
    fn test_embedding_with_dimensions_ok() {
        let emb =
            Embedding::with_dimensions(vec![1.0, 2.0, 3.0], 3, HashMap::new()).unwrap();
        assert_eq!(emb.dimensions, 3);
    }

    #[test]
    fn test_embedding_with_dimensions_mismatch() {
        let result = Embedding::with_dimensions(vec![1.0, 2.0], 3, HashMap::new());
        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::DimensionMismatch { expected, actual } => {
                assert_eq!(expected, 3);
                assert_eq!(actual, 2);
            }
            _ => panic!("Expected DimensionMismatch error"),
        }
    }

    #[test]
    fn test_embedding_error_display() {
        let err = EmbeddingError::FieldNotFound {
            field: "pm25".to_string(),
        };
        assert_eq!(err.to_string(), "Field 'pm25' not found in GoldRow");

        let err = EmbeddingError::InsufficientData {
            reason: "warmup incomplete".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Insufficient data for embedding: warmup incomplete"
        );
    }

    #[test]
    fn test_embedder_trait_is_object_safe() {
        // Verify the Embedder trait can be used as a trait object
        fn _accept_embedder(_e: &dyn Embedder) {}
    }
}
