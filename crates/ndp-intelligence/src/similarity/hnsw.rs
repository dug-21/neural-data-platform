//! HNSW-based similarity engine using ruvector-core
//!
//! Feature-gated behind `#[cfg(feature = "ruvector")]`.
//! Wraps ruvector_core::VectorDB for fast in-memory approximate nearest neighbor search.

#![cfg(feature = "ruvector")]

use std::sync::Arc;

use tracing::info;

use super::{SearchQuery, SearchResult, SimilarityEngine, SimilarityError, VectorEntry};
use crate::storage::StorageBackend;

/// HNSW-based similarity engine wrapping ruvector-core.
///
/// Provides sub-millisecond approximate nearest neighbor search.
/// All data is held in memory; on restart, rebuild from StorageBackend.
pub struct HnswEngine {
    db: ruvector_core::VectorDB,
    dimensions: usize,
    count: usize,
}

impl HnswEngine {
    /// Create a new HnswEngine with the given dimensionality.
    pub fn new(dimensions: usize) -> Result<Self, SimilarityError> {
        let config = ruvector_core::VectorDBConfig {
            dimensions,
            distance_metric: ruvector_core::DistanceMetric::Cosine,
            ef_construction: 64,
            m: 16,
        };
        let db = ruvector_core::VectorDB::new(config)
            .map_err(|e| SimilarityError::Backend(format!("Failed to create HNSW index: {}", e)))?;
        Ok(Self {
            db,
            dimensions,
            count: 0,
        })
    }

    /// Rebuild the HNSW index from stored embeddings in the database.
    ///
    /// Called on startup to restore the in-memory index from durable storage.
    pub async fn rebuild_from_storage(
        &mut self,
        storage: &dyn StorageBackend,
        domain_id: &str,
    ) -> Result<usize, SimilarityError> {
        let embeddings = storage
            .load_embeddings(domain_id, None)
            .await
            .map_err(|e| SimilarityError::Backend(format!("Failed to load embeddings: {}", e)))?;
        let mut count = 0;
        for emb in embeddings {
            let entry = VectorEntry {
                id: format!("{}", emb.bucket.timestamp()),
                vector: emb.embedding,
                metadata: emb.metadata,
            };
            self.insert(entry)?;
            count += 1;
        }
        info!(
            "Rebuilt HNSW index with {} vectors for domain {}",
            count, domain_id
        );
        Ok(count)
    }
}

impl SimilarityEngine for HnswEngine {
    fn insert(&mut self, entry: VectorEntry) -> Result<(), SimilarityError> {
        if entry.vector.len() != self.dimensions {
            return Err(SimilarityError::DimensionMismatch {
                expected: self.dimensions,
                actual: entry.vector.len(),
            });
        }
        let ruv_entry = ruvector_core::VectorEntry {
            id: entry.id,
            vector: entry.vector,
            metadata: entry.metadata,
        };
        self.db
            .insert(ruv_entry)
            .map_err(|e| SimilarityError::Backend(format!("HNSW insert failed: {}", e)))?;
        self.count += 1;
        Ok(())
    }

    fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SimilarityError> {
        if self.count == 0 {
            return Ok(vec![]);
        }
        if query.vector.len() != self.dimensions {
            return Err(SimilarityError::DimensionMismatch {
                expected: self.dimensions,
                actual: query.vector.len(),
            });
        }
        let ruv_query = ruvector_core::SearchQuery {
            vector: query.vector.clone(),
            k: query.k,
        };
        let results = self
            .db
            .search(ruv_query)
            .map_err(|e| SimilarityError::Backend(format!("HNSW search failed: {}", e)))?;
        Ok(results
            .into_iter()
            .filter(|r| r.similarity >= query.min_similarity as f32)
            .map(|r| SearchResult {
                id: r.id,
                similarity: r.similarity as f64,
                metadata: r.metadata,
            })
            .collect())
    }

    fn count(&self) -> usize {
        self.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hnsw_new() {
        let engine = HnswEngine::new(4);
        assert!(engine.is_ok());
        let engine = engine.unwrap();
        assert_eq!(engine.count(), 0);
    }

    #[test]
    fn test_hnsw_insert_and_count() {
        let mut engine = HnswEngine::new(3).unwrap();
        let entry = VectorEntry {
            id: "v1".to_string(),
            vector: vec![1.0, 0.0, 0.0],
            metadata: serde_json::json!({}),
        };
        assert!(engine.insert(entry).is_ok());
        assert_eq!(engine.count(), 1);
    }

    #[test]
    fn test_hnsw_dimension_mismatch() {
        let mut engine = HnswEngine::new(3).unwrap();
        let entry = VectorEntry {
            id: "v1".to_string(),
            vector: vec![1.0, 0.0], // wrong dimensions
            metadata: serde_json::json!({}),
        };
        let result = engine.insert(entry);
        assert!(result.is_err());
        match result.unwrap_err() {
            SimilarityError::DimensionMismatch { expected, actual } => {
                assert_eq!(expected, 3);
                assert_eq!(actual, 2);
            }
            e => panic!("Expected DimensionMismatch, got {:?}", e),
        }
    }

    #[test]
    fn test_hnsw_search_empty_index() {
        let engine = HnswEngine::new(3).unwrap();
        let query = SearchQuery {
            vector: vec![1.0, 0.0, 0.0],
            k: 5,
            min_similarity: 0.0,
        };
        let results = engine.search(&query).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_hnsw_search_dimension_mismatch() {
        let mut engine = HnswEngine::new(3).unwrap();
        let entry = VectorEntry {
            id: "v1".to_string(),
            vector: vec![1.0, 0.0, 0.0],
            metadata: serde_json::json!({}),
        };
        engine.insert(entry).unwrap();

        let query = SearchQuery {
            vector: vec![1.0, 0.0], // wrong dimensions
            k: 5,
            min_similarity: 0.0,
        };
        let result = engine.search(&query);
        assert!(result.is_err());
    }
}
