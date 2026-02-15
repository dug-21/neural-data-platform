//! Intelligence service orchestrator
//!
//! Coordinates the full observe-embed-store-search-predict-evaluate cycle.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use deadpool_postgres::Pool;
use tracing::{debug, info, warn};

use crate::error::{IntelligenceError, Result};
use crate::predictions::outcome::OutcomeTracker;
use crate::predictions::{ObjectiveMetric, PredictionEngine};
use crate::similarity::{self, SearchQuery, SimilarityEngine, VectorEntry};
use crate::storage::{StorageBackend, StoredEmbedding};
use ndp_lib::gold::embeddings::config::{IntelligenceConfig, SearchConfig};
use ndp_lib::gold::embeddings::metric::MetricEmbedder;
use ndp_lib::gold::embeddings::{Embedder, GoldRow};

/// Summary of a single intelligence cycle.
#[derive(Debug, Default)]
pub struct CycleSummary {
    /// Number of Gold rows observed
    pub rows_observed: usize,
    /// Number of embeddings generated
    pub embeddings_generated: usize,
    /// Number of similar neighbors found
    pub neighbors_found: usize,
    /// Number of predictions made
    pub predictions_made: usize,
    /// Number of outcomes evaluated
    pub outcomes_evaluated: usize,
    /// Number of correct predictions
    pub correct: usize,
    /// Number of incorrect predictions
    pub incorrect: usize,
    /// Duration of the cycle
    pub duration: Duration,
}

impl std::fmt::Display for CycleSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CycleSummary {{ rows: {}, embeddings: {}, neighbors: {}, predictions: {}, \
             evaluated: {}, correct: {}, incorrect: {}, duration: {:?} }}",
            self.rows_observed,
            self.embeddings_generated,
            self.neighbors_found,
            self.predictions_made,
            self.outcomes_evaluated,
            self.correct,
            self.incorrect,
            self.duration
        )
    }
}

/// Configuration for the intelligence app runtime.
///
/// Loaded from environment variables at startup. Domain-specific intelligence
/// configuration (embedding fields, search params, objectives) comes from etcd.
pub struct AppConfig {
    /// PostgreSQL connection string
    pub database_url: String,
    /// Domain identifier (e.g., "indoor-air-quality")
    pub domain_id: String,
    /// etcd endpoints for config loading
    pub etcd_endpoints: Vec<String>,
    /// Poll interval in seconds (default: 1200 = 20 min)
    pub poll_interval_secs: u64,
    /// Connection pool size (default: 2)
    pub pool_size: usize,
    /// Warmup threshold (default: 168 observations)
    pub warmup_threshold: usize,
}

impl AppConfig {
    /// Load configuration from environment variables.
    pub fn from_env() -> Result<Self> {
        let database_url = std::env::var("DATABASE_URL").map_err(|_| IntelligenceError::Config {
            message: "DATABASE_URL environment variable is required".to_string(),
        })?;
        let domain_id =
            std::env::var("INTELLIGENCE_DOMAIN").map_err(|_| IntelligenceError::Config {
                message: "INTELLIGENCE_DOMAIN environment variable is required".to_string(),
            })?;
        let etcd_endpoints = std::env::var("ETCD_ENDPOINTS")
            .unwrap_or_else(|_| "http://etcd:2379".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        let poll_interval_secs = std::env::var("INTELLIGENCE_POLL_INTERVAL_SECS")
            .unwrap_or_else(|_| "1200".to_string())
            .parse()
            .unwrap_or(1200);
        let pool_size = std::env::var("INTELLIGENCE_POOL_SIZE")
            .unwrap_or_else(|_| "2".to_string())
            .parse()
            .unwrap_or(2);
        let warmup_threshold = std::env::var("INTELLIGENCE_WARMUP_THRESHOLD")
            .unwrap_or_else(|_| "168".to_string())
            .parse()
            .unwrap_or(168);

        Ok(Self {
            database_url,
            domain_id,
            etcd_endpoints,
            poll_interval_secs,
            pool_size,
            warmup_threshold,
        })
    }
}

/// Orchestrates the full intelligence cycle: observe-embed-store-search-predict-evaluate.
pub struct IntelligenceService {
    similarity: Box<dyn SimilarityEngine>,
    storage: Arc<dyn StorageBackend>,
    embedder: MetricEmbedder,
    prediction_engine: PredictionEngine,
    outcome_tracker: OutcomeTracker,
    db_pool: Arc<Pool>,
    domain_id: String,
    search_config: SearchConfig,
    last_processed: Option<DateTime<Utc>>,
    observation_count: usize,
    warmup_threshold: usize,
    backfill_mode: bool,
}

impl IntelligenceService {
    /// Create a new IntelligenceService, rebuilding state from the database.
    ///
    /// On startup:
    /// 1. Queries observation count from gold.metric_embeddings
    /// 2. Replays Gold aligned view rows to rebuild MetricEmbedder running stats
    /// 3. Creates similarity engine (rebuilds HNSW index if ruvector enabled)
    /// 4. Finds last processed bucket
    pub async fn new(
        app_config: &AppConfig,
        intelligence_config: &IntelligenceConfig,
        objectives: Vec<ObjectiveMetric>,
        pool: Arc<Pool>,
        storage: Arc<dyn StorageBackend>,
    ) -> Result<Self> {
        // Build embedder from config
        let mut embedder = MetricEmbedder::from_config(&intelligence_config.embedding)
            .with_warmup(app_config.warmup_threshold);

        let dimensions = embedder.dimensions();

        // Rebuild observation count from database (ADR-013)
        let client = pool
            .get()
            .await
            .map_err(|e| IntelligenceError::Database(format!("Pool error: {}", e)))?;

        let count_row = client
            .query_one(
                "SELECT count(*)::bigint FROM gold.metric_embeddings WHERE domain_id = $1",
                &[&app_config.domain_id],
            )
            .await
            .map_err(|e| IntelligenceError::Database(format!("Count query error: {}", e)))?;
        let observation_count = count_row.get::<_, i64>(0) as usize;

        // Rebuild running stats by replaying Gold aligned view data
        let view_name = format!(
            "gold.{}_aligned_hourly",
            app_config.domain_id.replace('-', "_")
        );
        let rows = client
            .query(
                &format!("SELECT * FROM {} ORDER BY bucket ASC", view_name),
                &[],
            )
            .await
            .map_err(|e| {
                IntelligenceError::Database(format!(
                    "Failed to query {} for warmup: {}",
                    view_name, e
                ))
            })?;

        for row in &rows {
            let gold_row = sql_row_to_gold_row(row, &app_config.domain_id);
            embedder.observe(&gold_row);
        }

        // Create similarity engine via factory
        let similarity_engine = similarity::create_similarity_engine(
            intelligence_config,
            storage.clone(),
            pool.clone(),
            dimensions,
            &app_config.domain_id,
        )
        .await
        .map_err(IntelligenceError::Similarity)?;

        // Create prediction engine
        let prediction_engine = PredictionEngine::new(pool.clone(), intelligence_config, &objectives);

        // Create outcome tracker
        let outcome_tracker =
            OutcomeTracker::new(pool.clone(), storage.clone(), objectives);

        // Find last processed bucket
        let last_row = client
            .query_opt(
                "SELECT MAX(bucket) FROM gold.metric_embeddings WHERE domain_id = $1",
                &[&app_config.domain_id],
            )
            .await
            .map_err(|e| IntelligenceError::Database(format!("Last bucket query error: {}", e)))?;
        let last_processed: Option<DateTime<Utc>> = last_row.and_then(|r| r.get(0));

        info!(
            "IntelligenceService initialized: domain={}, observations={}, dimensions={}, warmed_up={}",
            app_config.domain_id,
            observation_count,
            dimensions,
            observation_count >= app_config.warmup_threshold
        );

        Ok(Self {
            similarity: similarity_engine,
            storage,
            embedder,
            prediction_engine,
            outcome_tracker,
            db_pool: pool,
            domain_id: app_config.domain_id.clone(),
            search_config: intelligence_config.search.clone(),
            last_processed,
            observation_count,
            warmup_threshold: app_config.warmup_threshold,
            backfill_mode: false,
        })
    }

    /// Run a single intelligence cycle.
    ///
    /// The cycle:
    /// 1. OBSERVE: query new Gold rows
    /// 2. WARMUP: feed to running stats
    /// 3. EMBED: generate embedding if warmed up
    /// 4. STORE: write to pgvector via StorageBackend
    /// 5. INDEX: insert into HNSW
    /// 6. SEARCH: find similar past states (skip in backfill)
    /// 7. PREDICT: generate predictions
    /// 8. EVALUATE: check pending predictions
    pub async fn run_cycle(&mut self) -> Result<CycleSummary> {
        let start = Instant::now();
        let mut summary = CycleSummary::default();

        // 1. OBSERVE: query new Gold rows
        let client = self
            .db_pool
            .get()
            .await
            .map_err(|e| IntelligenceError::Database(format!("Pool error: {}", e)))?;

        let view_name = format!(
            "gold.{}_aligned_hourly",
            self.domain_id.replace('-', "_")
        );

        let rows = if let Some(last) = self.last_processed {
            client
                .query(
                    &format!(
                        "SELECT * FROM {} WHERE bucket > $1 ORDER BY bucket ASC LIMIT 100",
                        view_name
                    ),
                    &[&last],
                )
                .await
                .map_err(|e| IntelligenceError::Database(format!("Query error: {}", e)))?
        } else {
            client
                .query(
                    &format!(
                        "SELECT * FROM {} ORDER BY bucket ASC LIMIT 100",
                        view_name
                    ),
                    &[],
                )
                .await
                .map_err(|e| IntelligenceError::Database(format!("Query error: {}", e)))?
        };

        if rows.is_empty() {
            debug!("No new rows to process");
            summary.duration = start.elapsed();
            return Ok(summary);
        }

        for row in &rows {
            let gold_row = sql_row_to_gold_row(row, &self.domain_id);
            summary.rows_observed += 1;

            // 2. WARMUP: observe for running stats
            self.embedder.observe(&gold_row);
            self.observation_count += 1;

            // 3. EMBED: generate embedding if warmed up
            if self.is_warmed_up() {
                match self.embedder.embed(&gold_row) {
                    Ok(embedding) => {
                        // 4. STORE: write to pgvector via StorageBackend
                        let stored = StoredEmbedding {
                            bucket: gold_row.bucket,
                            domain_id: self.domain_id.clone(),
                            embedding: embedding.vector.clone(),
                            dimensions: embedding.dimensions,
                            metadata: serde_json::json!({}),
                            created_at: Utc::now(),
                        };
                        if let Err(e) = self.storage.store_embedding(&stored).await {
                            warn!(
                                "Failed to store embedding for bucket {}: {}",
                                gold_row.bucket, e
                            );
                        } else {
                            summary.embeddings_generated += 1;
                        }

                        // 5. INDEX: insert into HNSW
                        let entry = VectorEntry {
                            id: format!("{}", gold_row.bucket.timestamp()),
                            vector: embedding.vector,
                            metadata: serde_json::json!({}),
                        };
                        if let Err(e) = self.similarity.insert(entry) {
                            warn!(
                                "HNSW insert failed for bucket {}: {}",
                                gold_row.bucket, e
                            );
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Embedding failed for bucket {}: {}",
                            gold_row.bucket, e
                        );
                    }
                }
            }

            // Track last processed
            self.last_processed = Some(gold_row.bucket);
        }

        // 6. SEARCH: find similar past states (skip in backfill mode)
        if !self.backfill_mode && self.is_warmed_up() && summary.embeddings_generated > 0 {
            let latest_bucket = self.last_processed.expect("last_processed must be set");
            let latest_emb = self
                .storage
                .load_embeddings(&self.domain_id, Some(latest_bucket - chrono::Duration::seconds(1)))
                .await
                .map_err(IntelligenceError::Storage)?;

            if let Some(emb) = latest_emb.first() {
                let query = SearchQuery {
                    vector: emb.embedding.clone(),
                    k: self.search_config.k,
                    min_similarity: self.search_config.min_similarity,
                };
                match self.similarity.search(&query) {
                    Ok(neighbors) => {
                        summary.neighbors_found = neighbors.len();

                        // 7. PREDICT: generate predictions
                        let predictions = self
                            .prediction_engine
                            .generate_predictions(latest_bucket, &self.domain_id, &neighbors)
                            .await?;
                        for pred in &predictions {
                            if let Err(e) = self.storage.store_prediction(pred).await {
                                warn!("Failed to store prediction: {}", e);
                            }
                        }
                        summary.predictions_made = predictions.len();
                    }
                    Err(e) => {
                        warn!("Search failed: {}", e);
                    }
                }
            }
        }

        // 8. EVALUATE: check pending predictions
        if !self.backfill_mode {
            let eval = self
                .outcome_tracker
                .evaluate_pending(&self.domain_id)
                .await?;
            summary.outcomes_evaluated = eval.evaluated;
            summary.correct = eval.correct;
            summary.incorrect = eval.incorrect;
        }

        summary.duration = start.elapsed();
        info!("Cycle complete: {}", summary);
        Ok(summary)
    }

    /// Check if the service has observed enough data for predictions.
    pub fn is_warmed_up(&self) -> bool {
        self.observation_count >= self.warmup_threshold
    }

    /// Set backfill mode (embed-only, no predictions or evaluations).
    pub fn set_backfill_mode(&mut self, backfill: bool) {
        self.backfill_mode = backfill;
    }

    /// Set the last processed bucket timestamp.
    pub fn set_last_processed(&mut self, last: Option<DateTime<Utc>>) {
        self.last_processed = last;
    }

    /// Get the current observation count.
    pub fn observation_count(&self) -> usize {
        self.observation_count
    }
}

/// Convert a tokio_postgres::Row from the Gold aligned view to a GoldRow.
pub fn sql_row_to_gold_row(row: &tokio_postgres::Row, domain_id: &str) -> GoldRow {
    let bucket: DateTime<Utc> = row.get("bucket");
    let mut fields = BTreeMap::new();
    for (idx, column) in row.columns().iter().enumerate() {
        if column.name() == "bucket" {
            continue;
        }
        // Try to read as f64
        let value: Option<f64> = row.try_get(idx).ok();
        fields.insert(column.name().to_string(), value);
    }
    GoldRow {
        bucket,
        domain_id: domain_id.to_string(),
        fields,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cycle_summary_default() {
        let summary = CycleSummary::default();
        assert_eq!(summary.rows_observed, 0);
        assert_eq!(summary.embeddings_generated, 0);
        assert_eq!(summary.neighbors_found, 0);
        assert_eq!(summary.predictions_made, 0);
        assert_eq!(summary.outcomes_evaluated, 0);
    }

    #[test]
    fn test_cycle_summary_display() {
        let summary = CycleSummary {
            rows_observed: 10,
            embeddings_generated: 8,
            neighbors_found: 5,
            predictions_made: 3,
            outcomes_evaluated: 2,
            correct: 1,
            incorrect: 1,
            duration: Duration::from_millis(250),
        };
        let s = format!("{}", summary);
        assert!(s.contains("rows: 10"));
        assert!(s.contains("embeddings: 8"));
        assert!(s.contains("predictions: 3"));
    }

    #[test]
    fn test_app_config_from_env_requires_database_url() {
        // Clear any existing env vars
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("INTELLIGENCE_DOMAIN");

        let result = AppConfig::from_env();
        assert!(result.is_err());
    }

    #[test]
    fn test_app_config_from_env_requires_domain() {
        std::env::set_var("DATABASE_URL", "postgresql://test");
        std::env::remove_var("INTELLIGENCE_DOMAIN");

        let result = AppConfig::from_env();
        assert!(result.is_err());

        std::env::remove_var("DATABASE_URL");
    }

    #[test]
    fn test_app_config_from_env_defaults() {
        std::env::set_var("DATABASE_URL", "postgresql://test");
        std::env::set_var("INTELLIGENCE_DOMAIN", "test-domain");
        // Clear optional vars to test defaults
        std::env::remove_var("ETCD_ENDPOINTS");
        std::env::remove_var("INTELLIGENCE_POLL_INTERVAL_SECS");
        std::env::remove_var("INTELLIGENCE_POOL_SIZE");
        std::env::remove_var("INTELLIGENCE_WARMUP_THRESHOLD");

        let config = AppConfig::from_env().unwrap();
        assert_eq!(config.database_url, "postgresql://test");
        assert_eq!(config.domain_id, "test-domain");
        assert_eq!(config.etcd_endpoints, vec!["http://etcd:2379"]);
        assert_eq!(config.poll_interval_secs, 1200);
        assert_eq!(config.pool_size, 2);
        assert_eq!(config.warmup_threshold, 168);

        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("INTELLIGENCE_DOMAIN");
    }
}
