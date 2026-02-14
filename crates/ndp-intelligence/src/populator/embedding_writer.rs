//! EmbeddingWriter — bridges Embedder and StorageBackend
//!
//! Takes Gold rows, generates embeddings via an Embedder, and writes
//! them to a StorageBackend.

use chrono::Utc;

use ndp_lib::gold::embeddings::{Embedder, GoldRow};

use crate::storage::{StorageBackend, StorageError, StoredEmbedding};

/// Writes embeddings from an Embedder to a StorageBackend.
pub struct EmbeddingWriter<S: StorageBackend> {
    storage: S,
}

impl<S: StorageBackend> EmbeddingWriter<S> {
    /// Create a new EmbeddingWriter with the given storage backend.
    pub fn new(storage: S) -> Self {
        Self { storage }
    }

    /// Write a single embedding for a Gold row.
    ///
    /// Uses the provided embedder to generate the vector, then stores it.
    pub async fn write_one(
        &self,
        embedder: &dyn Embedder,
        row: &GoldRow,
    ) -> Result<(), WriteError> {
        let embedding = embedder
            .embed(row)
            .map_err(|e| WriteError::Embedding(e.to_string()))?;

        let stored = StoredEmbedding {
            bucket: row.bucket,
            domain_id: row.domain_id.clone(),
            embedding: embedding.vector,
            dimensions: embedding.dimensions,
            metadata: serde_json::json!({}),
            created_at: Utc::now(),
        };

        self.storage
            .store_embedding(&stored)
            .await
            .map_err(WriteError::Storage)?;

        Ok(())
    }

    /// Write embeddings for a batch of Gold rows.
    ///
    /// Returns the count of successfully written embeddings.
    /// Continues on individual embedding errors, logging them.
    pub async fn write_batch(
        &self,
        embedder: &dyn Embedder,
        rows: &[GoldRow],
    ) -> Result<usize, WriteError> {
        let mut success_count = 0;

        for row in rows {
            match self.write_one(embedder, row).await {
                Ok(()) => success_count += 1,
                Err(WriteError::Embedding(msg)) => {
                    tracing::warn!(
                        domain_id = %row.domain_id,
                        bucket = %row.bucket,
                        error = %msg,
                        "Skipping row: embedding generation failed"
                    );
                }
                Err(e) => return Err(e),
            }
        }

        Ok(success_count)
    }

    /// Get a reference to the underlying storage backend.
    pub fn storage(&self) -> &S {
        &self.storage
    }
}

/// Errors from the embedding writer.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    /// Embedding generation failed
    #[error("Embedding error: {0}")]
    Embedding(String),

    /// Storage backend failed
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{ActualOutcome, Prediction, StoredEmbedding};
    use chrono::{DateTime, Utc};
    use ndp_lib::gold::embeddings::{Embedding, EmbeddingError, EmbeddingResult};
    use std::collections::{BTreeMap, HashMap};
    use std::sync::Mutex;

    // Mock Embedder for testing
    struct MockEmbedder {
        dimensions: usize,
        should_fail: bool,
    }

    impl Embedder for MockEmbedder {
        fn embed(&self, _row: &GoldRow) -> EmbeddingResult<Embedding> {
            if self.should_fail {
                return Err(EmbeddingError::InsufficientData {
                    reason: "mock failure".to_string(),
                });
            }
            Embedding::new(vec![0.1; self.dimensions], HashMap::new())
        }

        fn dimensions(&self) -> usize {
            self.dimensions
        }

        fn name(&self) -> &str {
            "MockEmbedder"
        }
    }

    // Mock StorageBackend for testing
    struct MockStorage {
        stored: Mutex<Vec<StoredEmbedding>>,
    }

    impl MockStorage {
        fn new() -> Self {
            Self {
                stored: Mutex::new(Vec::new()),
            }
        }

        fn count(&self) -> usize {
            self.stored.lock().unwrap().len()
        }
    }

    #[async_trait::async_trait]
    impl StorageBackend for MockStorage {
        async fn store_embedding(&self, embedding: &StoredEmbedding) -> Result<(), StorageError> {
            self.stored.lock().unwrap().push(embedding.clone());
            Ok(())
        }

        async fn load_embeddings(
            &self,
            _domain_id: &str,
            _since: Option<DateTime<Utc>>,
        ) -> Result<Vec<StoredEmbedding>, StorageError> {
            Ok(self.stored.lock().unwrap().clone())
        }

        async fn store_prediction(&self, _prediction: &Prediction) -> Result<i64, StorageError> {
            Ok(1)
        }

        async fn get_pending_outcomes(
            &self,
            _domain_id: &str,
        ) -> Result<Vec<Prediction>, StorageError> {
            Ok(vec![])
        }

        async fn record_outcome(
            &self,
            _prediction_id: i64,
            _actual: &ActualOutcome,
        ) -> Result<(), StorageError> {
            Ok(())
        }
    }

    fn make_test_row() -> GoldRow {
        let mut fields = BTreeMap::new();
        fields.insert("pm25".to_string(), Some(25.0));
        GoldRow {
            bucket: Utc::now(),
            domain_id: "test".to_string(),
            fields,
        }
    }

    #[tokio::test]
    async fn test_write_one_success() {
        let storage = MockStorage::new();
        let writer = EmbeddingWriter::new(storage);
        let embedder = MockEmbedder {
            dimensions: 3,
            should_fail: false,
        };

        let row = make_test_row();
        let result = writer.write_one(&embedder, &row).await;
        assert!(result.is_ok());
        assert_eq!(writer.storage().count(), 1);
    }

    #[tokio::test]
    async fn test_write_one_embedding_failure() {
        let storage = MockStorage::new();
        let writer = EmbeddingWriter::new(storage);
        let embedder = MockEmbedder {
            dimensions: 3,
            should_fail: true,
        };

        let row = make_test_row();
        let result = writer.write_one(&embedder, &row).await;
        assert!(result.is_err());
        assert_eq!(writer.storage().count(), 0);
    }

    #[tokio::test]
    async fn test_write_batch() {
        let storage = MockStorage::new();
        let writer = EmbeddingWriter::new(storage);
        let embedder = MockEmbedder {
            dimensions: 3,
            should_fail: false,
        };

        let rows: Vec<GoldRow> = (0..10).map(|_| make_test_row()).collect();
        let count = writer.write_batch(&embedder, &rows).await.unwrap();
        assert_eq!(count, 10);
        assert_eq!(writer.storage().count(), 10);
    }

    #[tokio::test]
    async fn test_write_batch_skips_embedding_failures() {
        let storage = MockStorage::new();
        let writer = EmbeddingWriter::new(storage);
        let embedder = MockEmbedder {
            dimensions: 3,
            should_fail: true,
        };

        let rows: Vec<GoldRow> = (0..5).map(|_| make_test_row()).collect();
        let count = writer.write_batch(&embedder, &rows).await.unwrap();
        assert_eq!(count, 0);
        assert_eq!(writer.storage().count(), 0);
    }
}
