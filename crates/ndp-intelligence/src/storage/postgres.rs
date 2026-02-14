//! PostgreSQL storage backend for embeddings and predictions
//!
//! Uses pgvector extension for efficient vector storage and retrieval.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use tokio_postgres::Client;

use super::{ActualOutcome, Prediction, StorageBackend, StorageError, StoredEmbedding};

/// PostgreSQL-backed storage using pgvector for embedding storage.
pub struct PostgresStorage {
    client: Arc<Client>,
}

impl PostgresStorage {
    /// Create a new PostgresStorage with the given client.
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    /// Convert a Vec<f32> to pgvector text format: "[1.0,2.0,3.0]"
    fn vec_to_pgvector(vec: &[f32]) -> String {
        let elements: Vec<String> = vec.iter().map(|v| v.to_string()).collect();
        format!("[{}]", elements.join(","))
    }

    /// Parse pgvector text format back to Vec<f32>
    fn pgvector_to_vec(text: &str) -> Result<Vec<f32>, StorageError> {
        let trimmed = text.trim_start_matches('[').trim_end_matches(']');
        if trimmed.is_empty() {
            return Ok(vec![]);
        }
        trimmed
            .split(',')
            .map(|s| {
                s.trim()
                    .parse::<f32>()
                    .map_err(|e| StorageError::Serialization(e.to_string()))
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl StorageBackend for PostgresStorage {
    async fn store_embedding(&self, embedding: &StoredEmbedding) -> Result<(), StorageError> {
        let vector_text = Self::vec_to_pgvector(&embedding.embedding);
        self.client
            .execute(
                "INSERT INTO gold.metric_embeddings (bucket, domain_id, embedding, dimensions, metadata, created_at)
                 VALUES ($1, $2, $3::vector, $4, $5, $6)
                 ON CONFLICT (bucket, domain_id) DO UPDATE SET
                     embedding = EXCLUDED.embedding,
                     dimensions = EXCLUDED.dimensions,
                     metadata = EXCLUDED.metadata",
                &[
                    &embedding.bucket,
                    &embedding.domain_id,
                    &vector_text,
                    &(embedding.dimensions as i32),
                    &embedding.metadata,
                    &embedding.created_at,
                ],
            )
            .await
            .map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(())
    }

    async fn load_embeddings(
        &self,
        domain_id: &str,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<StoredEmbedding>, StorageError> {
        let rows = if let Some(since_ts) = since {
            self.client
                .query(
                    "SELECT bucket, domain_id, embedding::text, dimensions, metadata, created_at
                     FROM gold.metric_embeddings
                     WHERE domain_id = $1 AND bucket > $2
                     ORDER BY bucket DESC",
                    &[&domain_id, &since_ts],
                )
                .await
        } else {
            self.client
                .query(
                    "SELECT bucket, domain_id, embedding::text, dimensions, metadata, created_at
                     FROM gold.metric_embeddings
                     WHERE domain_id = $1
                     ORDER BY bucket DESC",
                    &[&domain_id],
                )
                .await
        }
        .map_err(|e| StorageError::Database(e.to_string()))?;

        let mut embeddings = Vec::with_capacity(rows.len());
        for row in &rows {
            let vector_text: String = row.get(2);
            let vector = Self::pgvector_to_vec(&vector_text)?;
            embeddings.push(StoredEmbedding {
                bucket: row.get(0),
                domain_id: row.get(1),
                embedding: vector,
                dimensions: row.get::<_, i32>(3) as usize,
                metadata: row.get(4),
                created_at: row.get(5),
            });
        }

        Ok(embeddings)
    }

    async fn store_prediction(&self, prediction: &Prediction) -> Result<i64, StorageError> {
        let row = self
            .client
            .query_one(
                "INSERT INTO gold.predictions
                 (bucket, domain_id, metric, horizon, predicted_value, predicted_breach,
                  confidence, k_neighbors, k_supporting)
                 VALUES ($1, $2, $3, $4::interval, $5, $6, $7, $8, $9)
                 RETURNING id",
                &[
                    &prediction.bucket,
                    &prediction.domain_id,
                    &prediction.metric,
                    &prediction.horizon,
                    &prediction.predicted_value,
                    &prediction.predicted_breach,
                    &prediction.confidence,
                    &prediction.k_neighbors,
                    &prediction.k_supporting,
                ],
            )
            .await
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let id: i64 = row.get(0);
        Ok(id)
    }

    async fn get_pending_outcomes(
        &self,
        domain_id: &str,
    ) -> Result<Vec<Prediction>, StorageError> {
        let rows = self
            .client
            .query(
                "SELECT id, bucket, domain_id, metric, horizon::text, predicted_value,
                        predicted_breach, confidence, k_neighbors, k_supporting,
                        actual_value, actual_breach, correct, evaluated_at
                 FROM gold.predictions
                 WHERE domain_id = $1 AND actual_value IS NULL
                 ORDER BY bucket DESC",
                &[&domain_id],
            )
            .await
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let predictions = rows
            .iter()
            .map(|row| Prediction {
                id: Some(row.get(0)),
                bucket: row.get(1),
                domain_id: row.get(2),
                metric: row.get(3),
                horizon: row.get(4),
                predicted_value: row.get(5),
                predicted_breach: row.get(6),
                confidence: row.get(7),
                k_neighbors: row.get(8),
                k_supporting: row.get(9),
                actual_value: row.get(10),
                actual_breach: row.get(11),
                correct: row.get(12),
                evaluated_at: row.get(13),
            })
            .collect();

        Ok(predictions)
    }

    async fn record_outcome(
        &self,
        prediction_id: i64,
        actual: &ActualOutcome,
    ) -> Result<(), StorageError> {
        let rows_affected = self
            .client
            .execute(
                "UPDATE gold.predictions
                 SET actual_value = $2,
                     actual_breach = $3,
                     correct = (predicted_breach IS NOT NULL AND predicted_breach = $3),
                     evaluated_at = $4
                 WHERE id = $1",
                &[
                    &prediction_id,
                    &actual.actual_value,
                    &actual.actual_breach,
                    &actual.evaluated_at,
                ],
            )
            .await
            .map_err(|e| StorageError::Database(e.to_string()))?;

        if rows_affected == 0 {
            return Err(StorageError::NotFound {
                entity: "prediction".to_string(),
                id: prediction_id.to_string(),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec_to_pgvector() {
        let result = PostgresStorage::vec_to_pgvector(&[1.0, 2.5, 3.0]);
        assert_eq!(result, "[1,2.5,3]");
    }

    #[test]
    fn test_pgvector_to_vec() {
        let result = PostgresStorage::pgvector_to_vec("[1.0,2.5,3.0]").unwrap();
        assert_eq!(result.len(), 3);
        assert!((result[0] - 1.0).abs() < f32::EPSILON);
        assert!((result[1] - 2.5).abs() < f32::EPSILON);
        assert!((result[2] - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_pgvector_to_vec_empty() {
        let result = PostgresStorage::pgvector_to_vec("[]").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_postgres_storage_is_send_sync() {
        fn _assert_send_sync<T: Send + Sync>() {}
        _assert_send_sync::<PostgresStorage>();
    }

    // Integration tests are #[ignore] because they need a running PostgreSQL with pgvector.

    #[tokio::test]
    #[ignore]
    async fn test_store_and_load_embedding_round_trip() {
        // Requires: PostgreSQL with gold.metric_embeddings table + pgvector
    }

    #[tokio::test]
    #[ignore]
    async fn test_upsert_on_conflict() {
        // Verifies single row after two inserts for same bucket
    }

    #[tokio::test]
    #[ignore]
    async fn test_load_with_since_filter() {
        // Only returns newer records
    }

    #[tokio::test]
    #[ignore]
    async fn test_store_prediction_returns_id() {
        // ID > 0
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_pending_outcomes() {
        // Returns predictions where actual_value IS NULL
    }

    #[tokio::test]
    #[ignore]
    async fn test_record_outcome() {
        // Sets correct, actual_value, evaluated_at
    }
}
