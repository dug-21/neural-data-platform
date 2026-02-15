# fe-004 Pseudocode: Similarity Intelligence

> **Feature**: fe-004
> **Date**: 2026-02-15
> **References**: SPECIFICATION.md, ARCHITECTURE.md (ADRs 009-015), fe-003 types

---

## 1. HnswEngine (ruvector-core wrapper)

```
#[cfg(feature = "ruvector")]
struct HnswEngine {
    db: ruvector_core::VectorDB,
    dimensions: usize,
    count: usize,
}

fn HnswEngine::new(dimensions: usize) -> Result<Self>:
    let config = ruvector_core::VectorDBConfig {
        dimensions,
        distance_metric: DistanceMetric::Cosine,
        ef_construction: 64,
        m: 16,
    }
    let db = VectorDB::new(config)?
    return Self { db, dimensions, count: 0 }

async fn HnswEngine::rebuild_from_storage(
    &mut self, storage: &dyn StorageBackend, domain_id: &str
) -> Result<usize>:
    let embeddings = storage.load_embeddings(domain_id, None).await?
    let count = 0
    for emb in embeddings:
        let entry = VectorEntry {
            id: format!("{}", emb.bucket.timestamp()),
            vector: emb.embedding,
            metadata: emb.metadata,
        }
        self.insert(entry)?  // uses SimilarityEngine::insert
        count += 1
    info!("Rebuilt HNSW index with {} vectors for domain {}", count, domain_id)
    return Ok(count)

impl SimilarityEngine for HnswEngine:
    fn insert(&mut self, entry: VectorEntry) -> Result<(), SimilarityError>:
        if entry.vector.len() != self.dimensions:
            return Err(DimensionMismatch { expected: self.dimensions, actual: entry.vector.len() })
        self.db.insert(ruvector_entry_from(entry))?
        self.count += 1
        Ok(())

    fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SimilarityError>:
        if self.count == 0:
            return Ok(vec![])  // empty index, not an error
        if query.vector.len() != self.dimensions:
            return Err(DimensionMismatch { ... })
        let results = self.db.search(ruvector_query_from(query))?
        return Ok(results.iter()
            .filter(|r| r.similarity >= query.min_similarity)
            .map(|r| SearchResult { id: r.id, similarity: r.similarity, metadata: r.metadata })
            .collect())

    fn count(&self) -> usize:
        self.count
```

---

## 2. PgVectorEngine (SQL fallback)

```
struct PgVectorEngine {
    pool: Arc<Pool>,
    dimensions: usize,
    domain_id: String,
}

fn PgVectorEngine::new(pool: Arc<Pool>, dimensions: usize, domain_id: String) -> Self:
    Self { pool, dimensions, domain_id }

impl SimilarityEngine for PgVectorEngine:
    fn insert(&mut self, _entry: VectorEntry) -> Result<(), SimilarityError>:
        // No-op: embeddings already written via StorageBackend::store_embedding
        Ok(())

    fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SimilarityError>:
        let handle = tokio::runtime::Handle::current()
        handle.block_on(async {
            let client = self.pool.get().await.map_err(|e| Backend(e.to_string()))?
            let sql = "SELECT bucket::text AS id,
                              1.0 - (embedding <=> $1::vector) AS similarity,
                              metadata
                       FROM gold.metric_embeddings
                       WHERE domain_id = $2
                       ORDER BY embedding <=> $1::vector
                       LIMIT $3"
            let vector_str = format_pgvector(&query.vector)
            let rows = client.query(sql, &[&vector_str, &self.domain_id, &(query.k as i64)]).await?
            Ok(rows.iter()
                .map(|row| SearchResult {
                    id: row.get("id"),
                    similarity: row.get("similarity"),
                    metadata: row.get("metadata"),
                })
                .filter(|r| r.similarity >= query.min_similarity)
                .collect())
        })

    fn count(&self) -> usize:
        let handle = tokio::runtime::Handle::current()
        handle.block_on(async {
            let client = self.pool.get().await.ok()?
            let row = client.query_one(
                "SELECT count(*)::bigint FROM gold.metric_embeddings WHERE domain_id = $1",
                &[&self.domain_id]
            ).await.ok()?
            row.get::<_, i64>(0) as usize
        }).unwrap_or(0)
```

---

## 3. DualSimilarityEngine

```
#[cfg(feature = "ruvector")]
struct DualSimilarityEngine {
    hnsw: HnswEngine,
    // Note: pgvector writes handled separately by StorageBackend
}

impl SimilarityEngine for DualSimilarityEngine:
    fn insert(&mut self, entry: VectorEntry) -> Result<(), SimilarityError>:
        // Only insert into HNSW; pgvector write done by StorageBackend
        self.hnsw.insert(entry)

    fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SimilarityError>:
        // Always search HNSW (faster)
        self.hnsw.search(query)

    fn count(&self) -> usize:
        self.hnsw.count()
```

---

## 4. Factory Function

```
fn create_similarity_engine(
    config: &IntelligenceConfig,
    storage: Arc<dyn StorageBackend>,
    pool: Arc<Pool>,
    dimensions: usize,
    domain_id: &str,
) -> Result<Box<dyn SimilarityEngine>>:
    #[cfg(feature = "ruvector")]
    {
        let mut hnsw = HnswEngine::new(dimensions)?
        let count = hnsw.rebuild_from_storage(storage.as_ref(), domain_id).await?
        info!("Using DualSimilarityEngine (HNSW with {} vectors)", count)
        return Ok(Box::new(DualSimilarityEngine { hnsw }))
    }
    #[cfg(not(feature = "ruvector"))]
    {
        info!("Using PgVectorEngine (ruvector feature not enabled)")
        return Ok(Box::new(PgVectorEngine::new(pool, dimensions, domain_id.to_string())))
    }
```

---

## 5. PredictionEngine

```
struct PredictionEngine {
    db_pool: Arc<Pool>,
    horizons: Vec<chrono::Duration>,
    min_confidence: f64,
    objective_metrics: Vec<ObjectiveMetric>,
}

fn PredictionEngine::new(db_pool: Arc<Pool>, config: &IntelligenceConfig) -> Self:
    let horizons = config.search.prediction_horizons.iter()
        .map(|h| parse_horizon(h))
        .collect()
    let objectives = config.objectives.iter()
        .map(|o| ObjectiveMetric { field: o.field, threshold: o.threshold, direction: o.direction })
        .collect()
    Self { db_pool, horizons, min_confidence: 0.5, objective_metrics: objectives }

async fn PredictionEngine::generate_predictions(
    &self,
    current_bucket: DateTime<Utc>,
    domain_id: &str,
    neighbors: &[SearchResult],
) -> Result<Vec<Prediction>>:
    let mut predictions = Vec::new()
    let client = self.db_pool.get().await?
    let view_name = format!("gold.{}_aligned_hourly", domain_id.replace('-', '_"))

    for horizon in &self.horizons:
        for objective in &self.objective_metrics:
            let mut supporting = 0
            let mut total_with_outcome = 0

            for neighbor in neighbors:
                let neighbor_bucket = parse_bucket_from_id(&neighbor.id)?
                let future_bucket = neighbor_bucket + *horizon

                // Query what actually happened at neighbor_bucket + horizon
                let sql = format!(
                    "SELECT {} FROM {} WHERE bucket = $1 LIMIT 1",
                    objective.field, view_name
                )
                let row = client.query_opt(&sql, &[&future_bucket]).await?

                if let Some(row) = row:
                    let value: Option<f64> = row.get(0)
                    if let Some(v) = value:
                        total_with_outcome += 1
                        let breached = match objective.direction:
                            Above => v > objective.threshold,
                            Below => v < objective.threshold,
                        if breached:
                            supporting += 1

            if total_with_outcome >= 3:
                let confidence = supporting as f64 / total_with_outcome as f64
                if confidence >= self.min_confidence:
                    predictions.push(Prediction {
                        id: None,
                        bucket: current_bucket,
                        domain_id: domain_id.to_string(),
                        metric: objective.field.clone(),
                        horizon: format_duration(horizon),
                        predicted_value: None,
                        predicted_breach: Some(confidence > 0.5),
                        confidence,
                        k_neighbors: total_with_outcome as i32,
                        k_supporting: supporting as i32,
                        actual_value: None,
                        actual_breach: None,
                        correct: None,
                        evaluated_at: None,
                    })

    return Ok(predictions)
```

---

## 6. OutcomeTracker

```
struct OutcomeTracker {
    db_pool: Arc<Pool>,
    storage: Arc<dyn StorageBackend>,
}

async fn OutcomeTracker::evaluate_pending(
    &self, domain_id: &str
) -> Result<EvaluationSummary>:
    let pending = self.storage.get_pending_outcomes(domain_id).await?
    let client = self.db_pool.get().await?
    let view_name = format!("gold.{}_aligned_hourly", domain_id.replace('-', '_'))
    let now = Utc::now()
    let mut summary = EvaluationSummary { evaluated: 0, correct: 0, incorrect: 0 }

    for prediction in pending:
        let horizon = parse_duration(&prediction.horizon)?
        let outcome_time = prediction.bucket + horizon

        // Only evaluate if the horizon has elapsed
        if outcome_time > now:
            continue

        let sql = format!(
            "SELECT {} FROM {} WHERE bucket = $1 LIMIT 1",
            prediction.metric, view_name
        )
        let row = client.query_opt(&sql, &[&outcome_time]).await?

        if let Some(row) = row:
            let actual_value: Option<f64> = row.get(0)
            if let Some(value) = actual_value:
                // Determine if breach actually occurred
                // (need threshold from config; stored in prediction metadata or re-read config)
                let actual_breach = determine_breach(value, &prediction)
                let correct = prediction.predicted_breach == Some(actual_breach)

                self.storage.record_outcome(
                    prediction.id.unwrap(),
                    &ActualOutcome {
                        actual_value: value,
                        actual_breach,
                        evaluated_at: now,
                    }
                ).await?

                summary.evaluated += 1
                if correct:
                    summary.correct += 1
                else:
                    summary.incorrect += 1

    return Ok(summary)
```

---

## 7. IntelligenceService

```
struct IntelligenceService {
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

async fn IntelligenceService::new(
    app_config: &AppConfig,
    intelligence_config: &IntelligenceConfig,
    pool: Arc<Pool>,
    storage: Arc<dyn StorageBackend>,
) -> Result<Self>:
    // Build embedder from config
    let mut embedder = MetricEmbedder::from_config(&intelligence_config.embedding)
        .with_warmup(app_config.warmup_threshold)

    // Determine dimensions
    let dimensions = embedder.dimensions()

    // Rebuild observation count from database (ADR-013)
    let client = pool.get().await?
    let count_row = client.query_one(
        "SELECT count(*)::bigint FROM gold.metric_embeddings WHERE domain_id = $1",
        &[&app_config.domain_id]
    ).await?
    let observation_count = count_row.get::<_, i64>(0) as usize

    // Rebuild running stats by replaying historical data
    let embeddings = storage.load_embeddings(&app_config.domain_id, None).await?
    for emb in &embeddings:
        // Convert stored embedding back to GoldRow for observation
        // (we need to query Gold for the raw values, not the embedding)
        // Alternative: query Gold aligned view directly
    // SIMPLIFICATION: Query Gold view for historical rows to rebuild stats
    let view_name = format!("gold.{}_aligned_hourly", app_config.domain_id.replace('-', '_'))
    let rows = client.query(
        &format!("SELECT * FROM {} ORDER BY bucket ASC", view_name), &[]
    ).await?
    for row in &rows:
        let gold_row = sql_row_to_gold_row(row, &app_config.domain_id)
        embedder.observe(&gold_row)

    // Create similarity engine via factory
    let similarity = create_similarity_engine(
        intelligence_config, storage.clone(), pool.clone(), dimensions, &app_config.domain_id
    ).await?

    // Create prediction engine
    let prediction_engine = PredictionEngine::new(pool.clone(), intelligence_config)

    // Create outcome tracker
    let outcome_tracker = OutcomeTracker::new(pool.clone(), storage.clone())

    // Find last processed bucket
    let last_row = client.query_opt(
        "SELECT MAX(bucket) FROM gold.metric_embeddings WHERE domain_id = $1",
        &[&app_config.domain_id]
    ).await?
    let last_processed = last_row.and_then(|r| r.get(0))

    info!("IntelligenceService initialized: domain={}, observations={}, dimensions={}, warmed_up={}",
        app_config.domain_id, observation_count, dimensions, observation_count >= app_config.warmup_threshold)

    return Ok(Self { ... })

async fn IntelligenceService::run_cycle(&mut self) -> Result<CycleSummary>:
    let start = Instant::now()
    let mut summary = CycleSummary::default()

    // 1. OBSERVE: query new Gold rows
    let client = self.db_pool.get().await?
    let view_name = format!("gold.{}_aligned_hourly", self.domain_id.replace('-', '_'))
    let rows = if let Some(last) = self.last_processed:
        client.query(
            &format!("SELECT * FROM {} WHERE bucket > $1 ORDER BY bucket ASC LIMIT 100", view_name),
            &[&last]
        ).await?
    else:
        client.query(
            &format!("SELECT * FROM {} ORDER BY bucket ASC LIMIT 100", view_name),
            &[]
        ).await?

    if rows.is_empty():
        debug!("No new rows to process")
        return Ok(summary)

    for row in &rows:
        let gold_row = sql_row_to_gold_row(row, &self.domain_id)
        summary.rows_observed += 1

        // 2. WARMUP: observe for running stats
        self.embedder.observe(&gold_row)
        self.observation_count += 1

        // 3. EMBED: generate embedding if warmed up
        if self.is_warmed_up():
            match self.embedder.embed(&gold_row):
                Ok(embedding) =>
                    // 4. STORE: write to pgvector via StorageBackend
                    let stored = StoredEmbedding {
                        bucket: gold_row.bucket,
                        domain_id: self.domain_id.clone(),
                        embedding: embedding.vector.clone(),
                        dimensions: embedding.dimensions,
                        metadata: serde_json::json!({}),
                        created_at: Utc::now(),
                    }
                    self.storage.store_embedding(&stored).await?
                    summary.embeddings_generated += 1

                    // 5. INDEX: insert into HNSW
                    let entry = VectorEntry {
                        id: format!("{}", gold_row.bucket.timestamp()),
                        vector: embedding.vector,
                        metadata: serde_json::json!({}),
                    }
                    if let Err(e) = self.similarity.insert(entry):
                        warn!("HNSW insert failed for bucket {}: {}", gold_row.bucket, e)

                Err(e) =>
                    warn!("Embedding failed for bucket {}: {}", gold_row.bucket, e)

        // Track last processed
        self.last_processed = Some(gold_row.bucket)

    // 6. SEARCH: find similar past states (skip in backfill mode)
    if !self.backfill_mode && self.is_warmed_up() && summary.embeddings_generated > 0:
        // Use the last embedding generated
        let latest_bucket = self.last_processed.unwrap()
        let latest_emb = self.storage.load_embeddings(&self.domain_id, Some(latest_bucket)).await?
        if let Some(emb) = latest_emb.last():
            let query = SearchQuery {
                vector: emb.embedding.clone(),
                k: self.search_config.k,
                min_similarity: self.search_config.min_similarity,
            }
            match self.similarity.search(&query):
                Ok(neighbors) =>
                    summary.neighbors_found = neighbors.len()

                    // 7. PREDICT: generate predictions
                    let predictions = self.prediction_engine.generate_predictions(
                        latest_bucket, &self.domain_id, &neighbors
                    ).await?
                    for pred in &predictions:
                        self.storage.store_prediction(pred).await?
                    summary.predictions_made = predictions.len()

                Err(e) =>
                    warn!("Search failed: {}", e)

    // 8. EVALUATE: check pending predictions
    if !self.backfill_mode:
        let eval = self.outcome_tracker.evaluate_pending(&self.domain_id).await?
        summary.outcomes_evaluated = eval.evaluated
        summary.correct = eval.correct
        summary.incorrect = eval.incorrect

    summary.duration = start.elapsed()
    info!("Cycle complete: {:?}", summary)
    return Ok(summary)

fn IntelligenceService::is_warmed_up(&self) -> bool:
    self.observation_count >= self.warmup_threshold
```

---

## 8. NotifyListener

```
struct NotifyListener {
    connection_string: String,
    channel: String,
}

fn NotifyListener::new(connection_string: &str, channel: &str) -> Self:
    Self { connection_string: connection_string.to_string(), channel: channel.to_string() }

async fn NotifyListener::listen(&self) -> Result<mpsc::Receiver<String>>:
    let (tx, rx) = mpsc::channel(16)
    let conn_str = self.connection_string.clone()
    let channel = self.channel.clone()

    tokio::spawn(async move {
        let mut backoff = Duration::from_secs(1)
        loop:
            match connect_and_listen(&conn_str, &channel, &tx).await:
                Ok(()) => break,  // clean shutdown
                Err(e) =>
                    warn!("NOTIFY connection lost: {}. Retrying in {:?}", e, backoff)
                    tokio::time::sleep(backoff).await
                    backoff = (backoff * 2).min(Duration::from_secs(60))
    })

    return Ok(rx)

async fn connect_and_listen(
    conn_str: &str, channel: &str, tx: &mpsc::Sender<String>
) -> Result<()>:
    let (client, mut connection) = tokio_postgres::connect(conn_str, NoTls).await?
    client.execute(&format!("LISTEN {}", channel), &[]).await?
    info!("Listening on PG channel '{}'", channel)

    // tokio_postgres connection provides notifications
    loop:
        match connection.next_notification().await:
            Some(notification) =>
                tx.send(notification.payload().to_string()).await.ok()
            None =>
                return Err("Connection closed")
```

---

## 9. Daemon Main Loop

```
async fn run_daemon(app_config: AppConfig) -> Result<()>:
    // Initialize connection pool
    let pool = create_pool(&app_config)?

    // Load domain config from etcd via config-client
    let config_client = ConfigClient::new(&app_config.etcd_endpoints).await?
    let domain_config: DomainConfig = config_client
        .get(&format!("/domains/{}/config", app_config.domain_id)).await?
    let intel_config = domain_config.intelligence
        .ok_or(ConfigError("intelligence block not in domain config"))?

    // Create storage backend
    let storage = Arc::new(PostgresStorage::new(pool.clone()).await?)

    // Create intelligence service
    let mut service = IntelligenceService::new(&app_config, &intel_config, pool.clone(), storage).await?

    // Start NOTIFY listener (optional, non-fatal if fails)
    let notify_rx = match NotifyListener::new(&app_config.database_url, "gold_refresh").listen().await:
        Ok(rx) => Some(rx),
        Err(e) =>
            warn!("PG NOTIFY listener failed to start: {}. Using timer only.", e)
            None

    // Timer fallback
    let mut timer = tokio::time::interval(Duration::from_secs(app_config.poll_interval_secs))
    timer.tick().await  // first tick fires immediately

    // Shutdown signal
    let mut shutdown = tokio::signal::ctrl_c()

    info!("Intelligence daemon started for domain '{}'", app_config.domain_id)

    loop:
        tokio::select! {
            _ = &mut shutdown => {
                info!("Shutdown signal received")
                break
            }
            Some(payload) = async { notify_rx.as_mut()?.recv().await }, if notify_rx.is_some() => {
                debug!("PG NOTIFY received: {}", payload)
                match service.run_cycle().await:
                    Ok(summary) => info!("Cycle (NOTIFY): {:?}", summary),
                    Err(e) => error!("Cycle failed: {}", e),
            }
            _ = timer.tick() => {
                match service.run_cycle().await:
                    Ok(summary) => info!("Cycle (timer): {:?}", summary),
                    Err(e) => error!("Cycle failed: {}", e),
            }
        }

    info!("Intelligence daemon stopped")
    Ok(())
```

---

## 10. One-Shot Mode

```
async fn run_one_shot(app_config: AppConfig) -> Result<()>:
    let pool = create_pool(&app_config)?
    let config_client = ConfigClient::new(&app_config.etcd_endpoints).await?
    let domain_config: DomainConfig = config_client
        .get(&format!("/domains/{}/config", app_config.domain_id)).await?
    let intel_config = domain_config.intelligence
        .ok_or(ConfigError("intelligence block not in domain config"))?
    let storage = Arc::new(PostgresStorage::new(pool.clone()).await?)
    let mut service = IntelligenceService::new(&app_config, &intel_config, pool, storage).await?

    let summary = service.run_cycle().await?
    info!("One-shot cycle complete: {:?}", summary)
    Ok(())
```

---

## 11. Backfill Mode

```
async fn run_backfill(app_config: AppConfig, since: Option<DateTime<Utc>>) -> Result<()>:
    let pool = create_pool(&app_config)?
    let config_client = ConfigClient::new(&app_config.etcd_endpoints).await?
    let domain_config: DomainConfig = config_client
        .get(&format!("/domains/{}/config", app_config.domain_id)).await?
    let intel_config = domain_config.intelligence
        .ok_or(ConfigError("intelligence block not in domain config"))?
    let storage = Arc::new(PostgresStorage::new(pool.clone()).await?)
    let mut service = IntelligenceService::new(&app_config, &intel_config, pool, storage).await?
    service.set_backfill_mode(true)

    if let Some(since) = since:
        service.set_last_processed(Some(since - chrono::Duration::seconds(1)))

    let mut total_embeddings = 0
    loop:
        let summary = service.run_cycle().await?
        total_embeddings += summary.embeddings_generated
        if summary.rows_observed == 0:
            break  // no more rows to process

    info!("Backfill complete: {} total embeddings generated", total_embeddings)
    Ok(())
```

---

## 12. Helper Functions

```
fn sql_row_to_gold_row(row: &tokio_postgres::Row, domain_id: &str) -> GoldRow:
    let bucket: DateTime<Utc> = row.get("bucket")
    let mut fields = BTreeMap::new()
    for (idx, column) in row.columns().iter().enumerate():
        if column.name() == "bucket":
            continue
        // Try to read as f64
        let value: Option<f64> = row.try_get(idx).ok()
        fields.insert(column.name().to_string(), value)
    GoldRow { bucket, domain_id: domain_id.to_string(), fields }

fn parse_bucket_from_id(id: &str) -> Result<DateTime<Utc>>:
    let timestamp = id.parse::<i64>()?
    Ok(DateTime::from_timestamp(timestamp, 0).unwrap())

fn format_pgvector(vector: &[f32]) -> String:
    let vals: Vec<String> = vector.iter().map(|v| v.to_string()).collect()
    format!("[{}]", vals.join(","))

fn parse_horizon(s: &str) -> chrono::Duration:
    // Parse "1 hour", "4 hours", "24 hours" -> chrono::Duration
    let parts: Vec<&str> = s.split_whitespace().collect()
    let value: i64 = parts[0].parse().unwrap_or(1)
    match parts.get(1).map(|s| s.trim_end_matches('s')):
        Some("hour") => chrono::Duration::hours(value),
        Some("minute") => chrono::Duration::minutes(value),
        Some("day") => chrono::Duration::days(value),
        _ => chrono::Duration::hours(value),  // default to hours
```

---

## 13. AppConfig from Environment

```
fn AppConfig::from_env() -> Result<Self>:
    Ok(Self {
        database_url: env::var("DATABASE_URL")
            .map_err(|_| ConfigError("DATABASE_URL required".into()))?,
        domain_id: env::var("INTELLIGENCE_DOMAIN")
            .map_err(|_| ConfigError("INTELLIGENCE_DOMAIN required".into()))?,
        etcd_endpoints: env::var("ETCD_ENDPOINTS")
            .unwrap_or_else(|_| "http://etcd:2379".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .collect(),
        poll_interval_secs: env::var("INTELLIGENCE_POLL_INTERVAL_SECS")
            .unwrap_or_else(|_| "1200".to_string())
            .parse()
            .unwrap_or(1200),
        pool_size: env::var("INTELLIGENCE_POOL_SIZE")
            .unwrap_or_else(|_| "2".to_string())
            .parse()
            .unwrap_or(2),
        warmup_threshold: env::var("INTELLIGENCE_WARMUP_THRESHOLD")
            .unwrap_or_else(|_| "168".to_string())
            .parse()
            .unwrap_or(168),
    })
```
