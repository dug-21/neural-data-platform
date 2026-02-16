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
    ///
    /// Returns an error if any element is NaN or Infinity, which would
    /// produce invalid pgvector data and corrupt similarity searches.
    fn vec_to_pgvector(vec: &[f32]) -> Result<String, StorageError> {
        for (i, v) in vec.iter().enumerate() {
            if !v.is_finite() {
                return Err(StorageError::Serialization(format!(
                    "Non-finite value at index {}: {}",
                    i, v
                )));
            }
        }
        let elements: Vec<String> = vec.iter().map(|v| v.to_string()).collect();
        Ok(format!("[{}]", elements.join(",")))
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
        let vector_text = Self::vec_to_pgvector(&embedding.embedding)?;
        self.client
            .execute(
                "INSERT INTO gold.metric_embeddings (bucket, domain_id, embedding, dimensions, metadata, created_at)
                 VALUES ($1, $2, $3::text::vector, $4, $5, $6)
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
                 VALUES ($1, $2, $3, $4::text::interval, $5, $6, $7, $8, $9)
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
        let result = PostgresStorage::vec_to_pgvector(&[1.0, 2.5, 3.0]).unwrap();
        assert_eq!(result, "[1,2.5,3]");
    }

    #[test]
    fn test_vec_to_pgvector_rejects_nan() {
        let result = PostgresStorage::vec_to_pgvector(&[1.0, f32::NAN, 3.0]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Non-finite"), "Error: {}", err);
        assert!(err.to_string().contains("index 1"), "Error: {}", err);
    }

    #[test]
    fn test_vec_to_pgvector_rejects_infinity() {
        let result = PostgresStorage::vec_to_pgvector(&[f32::INFINITY, 2.0]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Non-finite"), "Error: {}", err);
    }

    #[test]
    fn test_vec_to_pgvector_rejects_neg_infinity() {
        let result = PostgresStorage::vec_to_pgvector(&[1.0, f32::NEG_INFINITY]);
        assert!(result.is_err());
    }

    #[test]
    fn test_vec_to_pgvector_empty() {
        let result = PostgresStorage::vec_to_pgvector(&[]).unwrap();
        assert_eq!(result, "[]");
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
    fn test_pgvector_to_vec_whitespace() {
        let result = PostgresStorage::pgvector_to_vec("[ 1.0 , 2.0 , 3.0 ]").unwrap();
        assert_eq!(result.len(), 3);
        assert!((result[0] - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_pgvector_to_vec_invalid_element() {
        let result = PostgresStorage::pgvector_to_vec("[1.0,abc,3.0]");
        assert!(result.is_err());
    }

    #[test]
    fn test_pgvector_round_trip() {
        let original = vec![0.1_f32, -2.5, 100.0, 0.0, -0.001];
        let text = PostgresStorage::vec_to_pgvector(&original).unwrap();
        let parsed = PostgresStorage::pgvector_to_vec(&text).unwrap();
        assert_eq!(original.len(), parsed.len());
        for (a, b) in original.iter().zip(parsed.iter()) {
            assert!((a - b).abs() < 1e-6, "Mismatch: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_postgres_storage_is_send_sync() {
        fn _assert_send_sync<T: Send + Sync>() {}
        _assert_send_sync::<PostgresStorage>();
    }

    // ---- Integration tests ----
    // Require a running PostgreSQL with pgvector extension.
    // Run with: TIMESCALE_URL="host=localhost dbname=ndp user=ndp password=ndp" cargo test -p ndp-intelligence -- --ignored

    async fn setup_integration_client() -> Arc<Client> {
        let url = std::env::var("TIMESCALE_URL")
            .unwrap_or_else(|_| "host=localhost dbname=ndp user=ndp password=ndp".to_string());
        let (client, connection) = tokio_postgres::connect(&url, tokio_postgres::NoTls)
            .await
            .expect("Failed to connect to PostgreSQL");
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });
        let client = Arc::new(client);

        // Ensure schema and tables exist
        client
            .batch_execute(
                "CREATE EXTENSION IF NOT EXISTS vector;
                 CREATE SCHEMA IF NOT EXISTS gold;
                 CREATE TABLE IF NOT EXISTS gold.metric_embeddings (
                     bucket TIMESTAMPTZ NOT NULL,
                     domain_id TEXT NOT NULL,
                     embedding vector,
                     dimensions INTEGER NOT NULL,
                     metadata JSONB DEFAULT '{}',
                     created_at TIMESTAMPTZ DEFAULT NOW(),
                     PRIMARY KEY (bucket, domain_id)
                 );
                 CREATE TABLE IF NOT EXISTS gold.predictions (
                     id BIGSERIAL,
                     bucket TIMESTAMPTZ NOT NULL,
                     domain_id TEXT NOT NULL,
                     metric TEXT NOT NULL,
                     horizon INTERVAL NOT NULL,
                     predicted_value DOUBLE PRECISION,
                     predicted_breach BOOLEAN,
                     confidence DOUBLE PRECISION,
                     k_neighbors INTEGER,
                     k_supporting INTEGER,
                     actual_value DOUBLE PRECISION,
                     actual_breach BOOLEAN,
                     correct BOOLEAN,
                     evaluated_at TIMESTAMPTZ,
                     created_at TIMESTAMPTZ DEFAULT NOW(),
                     PRIMARY KEY (id)
                 );",
            )
            .await
            .expect("Failed to create tables");

        // Clean test data
        client
            .batch_execute(
                "DELETE FROM gold.metric_embeddings WHERE domain_id LIKE 'test-%';
                 DELETE FROM gold.predictions WHERE domain_id LIKE 'test-%';",
            )
            .await
            .expect("Failed to clean test data");

        client
    }

    #[tokio::test]
    #[ignore]
    async fn test_store_and_load_embedding_round_trip() {
        let client = setup_integration_client().await;
        let storage = PostgresStorage::new(client);
        let now = Utc::now();

        let embedding = StoredEmbedding {
            bucket: now,
            domain_id: "test-roundtrip".to_string(),
            embedding: vec![0.1, 0.2, 0.3, 0.4],
            dimensions: 4,
            metadata: serde_json::json!({"source": "test"}),
            created_at: now,
        };

        storage.store_embedding(&embedding).await.unwrap();

        let loaded = storage
            .load_embeddings("test-roundtrip", None)
            .await
            .unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].domain_id, "test-roundtrip");
        assert_eq!(loaded[0].dimensions, 4);
        assert_eq!(loaded[0].embedding.len(), 4);
        for (a, b) in embedding.embedding.iter().zip(loaded[0].embedding.iter()) {
            assert!((a - b).abs() < 1e-6, "Vector mismatch: {} vs {}", a, b);
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_upsert_on_conflict() {
        let client = setup_integration_client().await;
        let storage = PostgresStorage::new(client.clone());
        let now = Utc::now();

        let emb1 = StoredEmbedding {
            bucket: now,
            domain_id: "test-upsert".to_string(),
            embedding: vec![1.0, 2.0],
            dimensions: 2,
            metadata: serde_json::json!({"version": 1}),
            created_at: now,
        };

        let emb2 = StoredEmbedding {
            bucket: now,
            domain_id: "test-upsert".to_string(),
            embedding: vec![3.0, 4.0],
            dimensions: 2,
            metadata: serde_json::json!({"version": 2}),
            created_at: now,
        };

        storage.store_embedding(&emb1).await.unwrap();
        storage.store_embedding(&emb2).await.unwrap();

        let loaded = storage.load_embeddings("test-upsert", None).await.unwrap();
        assert_eq!(loaded.len(), 1, "Upsert should produce single row");
        // Should have the second embedding's data
        assert!((loaded[0].embedding[0] - 3.0).abs() < 1e-6);
    }

    #[tokio::test]
    #[ignore]
    async fn test_load_with_since_filter() {
        let client = setup_integration_client().await;
        let storage = PostgresStorage::new(client);

        let old = chrono::Utc::now() - chrono::Duration::hours(2);
        let recent = chrono::Utc::now() - chrono::Duration::minutes(30);
        let cutoff = chrono::Utc::now() - chrono::Duration::hours(1);

        for (bucket, suffix) in [(old, "old"), (recent, "recent")] {
            let emb = StoredEmbedding {
                bucket,
                domain_id: "test-since".to_string(),
                embedding: vec![1.0],
                dimensions: 1,
                metadata: serde_json::json!({"label": suffix}),
                created_at: bucket,
            };
            storage.store_embedding(&emb).await.unwrap();
        }

        let filtered = storage
            .load_embeddings("test-since", Some(cutoff))
            .await
            .unwrap();
        assert_eq!(filtered.len(), 1, "Should only return records after cutoff");
    }

    #[tokio::test]
    #[ignore]
    async fn test_store_prediction_returns_id() {
        let client = setup_integration_client().await;
        let storage = PostgresStorage::new(client);

        let prediction = Prediction {
            id: None,
            bucket: Utc::now(),
            domain_id: "test-pred-id".to_string(),
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

        let id = storage.store_prediction(&prediction).await.unwrap();
        assert!(id > 0, "Prediction ID should be positive, got {}", id);
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_pending_outcomes() {
        let client = setup_integration_client().await;
        let storage = PostgresStorage::new(client);

        // Insert a prediction without actual_value
        let prediction = Prediction {
            id: None,
            bucket: Utc::now(),
            domain_id: "test-pending".to_string(),
            metric: "co2".to_string(),
            horizon: "1 hour".to_string(),
            predicted_value: Some(400.0),
            predicted_breach: Some(false),
            confidence: 0.9,
            k_neighbors: 5,
            k_supporting: 4,
            actual_value: None,
            actual_breach: None,
            correct: None,
            evaluated_at: None,
        };

        storage.store_prediction(&prediction).await.unwrap();

        let pending = storage
            .get_pending_outcomes("test-pending")
            .await
            .unwrap();
        assert!(
            !pending.is_empty(),
            "Should return at least one pending prediction"
        );
        assert!(
            pending.iter().all(|p| p.actual_value.is_none()),
            "All pending predictions should have actual_value IS NULL"
        );
    }

    #[tokio::test]
    #[ignore]
    async fn test_record_outcome() {
        let client = setup_integration_client().await;
        let storage = PostgresStorage::new(client);

        let prediction = Prediction {
            id: None,
            bucket: Utc::now(),
            domain_id: "test-outcome".to_string(),
            metric: "pm25".to_string(),
            horizon: "1 hour".to_string(),
            predicted_value: Some(30.0),
            predicted_breach: Some(true),
            confidence: 0.75,
            k_neighbors: 10,
            k_supporting: 7,
            actual_value: None,
            actual_breach: None,
            correct: None,
            evaluated_at: None,
        };

        let pred_id = storage.store_prediction(&prediction).await.unwrap();

        let outcome = ActualOutcome {
            actual_value: 32.0,
            actual_breach: true,
            evaluated_at: Utc::now(),
        };

        storage.record_outcome(pred_id, &outcome).await.unwrap();

        // Verify the prediction is no longer pending
        let pending = storage
            .get_pending_outcomes("test-outcome")
            .await
            .unwrap();
        assert!(
            pending.iter().all(|p| p.id != Some(pred_id)),
            "Recorded prediction should no longer be pending"
        );
    }
}
