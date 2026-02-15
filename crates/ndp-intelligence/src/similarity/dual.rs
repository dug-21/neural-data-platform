//! Dual similarity engine (HNSW wrapper)
//!
//! Feature-gated behind `#[cfg(feature = "ruvector")]`.
//! Wraps HnswEngine for reads and writes. pgvector writes are handled
//! separately by StorageBackend (see ADR-014).

#![cfg(feature = "ruvector")]

use super::hnsw::HnswEngine;
use super::{SearchQuery, SearchResult, SimilarityEngine, SimilarityError, VectorEntry};

/// Dual similarity engine that wraps HNSW for fast in-memory search.
///
/// pgvector writes are handled by StorageBackend, not by this engine.
/// This engine delegates all operations to the underlying HnswEngine.
pub struct DualSimilarityEngine {
    pub(crate) hnsw: HnswEngine,
}

impl DualSimilarityEngine {
    /// Create a new DualSimilarityEngine wrapping the given HNSW engine.
    pub fn new(hnsw: HnswEngine) -> Self {
        Self { hnsw }
    }
}

impl SimilarityEngine for DualSimilarityEngine {
    fn insert(&mut self, entry: VectorEntry) -> Result<(), SimilarityError> {
        // Only insert into HNSW; pgvector writes handled by StorageBackend
        self.hnsw.insert(entry)
    }

    fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SimilarityError> {
        // Always search HNSW (faster, sub-millisecond)
        self.hnsw.search(query)
    }

    fn count(&self) -> usize {
        self.hnsw.count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dual_engine_delegates_insert_to_hnsw() {
        let hnsw = HnswEngine::new(3).unwrap();
        let mut dual = DualSimilarityEngine::new(hnsw);

        let entry = VectorEntry {
            id: "v1".to_string(),
            vector: vec![1.0, 0.0, 0.0],
            metadata: serde_json::json!({}),
        };
        assert!(dual.insert(entry).is_ok());
        assert_eq!(dual.count(), 1);
    }

    #[test]
    fn test_dual_engine_delegates_search_to_hnsw() {
        let hnsw = HnswEngine::new(3).unwrap();
        let mut dual = DualSimilarityEngine::new(hnsw);

        let entry = VectorEntry {
            id: "v1".to_string(),
            vector: vec![1.0, 0.0, 0.0],
            metadata: serde_json::json!({}),
        };
        dual.insert(entry).unwrap();

        let query = SearchQuery {
            vector: vec![1.0, 0.0, 0.0],
            k: 5,
            min_similarity: 0.0,
        };
        let results = dual.search(&query).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_dual_engine_count_delegates() {
        let hnsw = HnswEngine::new(3).unwrap();
        let dual = DualSimilarityEngine::new(hnsw);
        assert_eq!(dual.count(), 0);
    }
}
