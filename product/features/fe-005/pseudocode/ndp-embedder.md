# fe-005 Pseudocode: ndp-embedder

## Location: `apps/ndp-embedder/` (NEW crate)

### `Cargo.toml`

```toml
[package]
name = "ndp-embedder"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "ndp-embedder"
path = "src/main.rs"

[dependencies]
ndp-lib = { path = "../../crates/ndp-lib" }
ndp-types = { path = "../../crates/ndp-types" }
config-client = { path = "../../config-client" }
tokio = { workspace = true }
tokio-postgres = { workspace = true }
deadpool-postgres = "0.14"
serde = { workspace = true }
serde_json = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
chrono = { workspace = true }
clap = { workspace = true }
```

### `src/main.rs`

```rust
use clap::Parser;
use std::path::PathBuf;
use std::time::Duration;
use tracing::{info, warn, error};

#[derive(Parser)]
#[command(name = "ndp-embedder", about = "Text embedding service for NDP")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Run the embedding daemon
    Daemon,
}

struct AppConfig {
    database_url: String,
    domain_id: String,
    etcd_endpoints: Vec<String>,
    model_volume_path: PathBuf,
    poll_interval_secs: u64,
    pool_size: usize,
}

impl AppConfig {
    fn from_env() -> Result<Self> {
        // Read from environment:
        // DATABASE_URL (required)
        // EMBEDDER_DOMAIN (required)
        // ETCD_ENDPOINTS (default: http://etcd:2379)
        // MODEL_VOLUME_PATH (default: /models)
        // EMBEDDER_POLL_INTERVAL_SECS (default: 1200)
        // EMBEDDER_POOL_SIZE (default: 2)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Daemon => run_daemon().await,
    }
}

async fn run_daemon() -> Result<()> {
    let config = AppConfig::from_env()?;
    info!("ndp-embedder starting: domain={}", config.domain_id);

    // 1. Create database connection pool
    let pool = create_pool(&config)?;

    // 2. Load domain config from etcd
    let domain_config = load_domain_config(&config).await?;

    // 3. Check for text_embedding config
    let text_config = match domain_config.text_embedding {
        Some(c) => c,
        None => {
            info!("No text_embedding config for domain '{}', exiting", config.domain_id);
            return Ok(());
        }
    };

    // 4. Ensure model is available (download if needed)
    let model_manager = ModelManager::new(&config.model_volume_path);
    let model_paths = model_manager.ensure_model(&text_config.model).await?;

    // 5. Load OnnxEmbedder
    let embedder = OnnxEmbedder::new(
        &model_paths.model,
        &model_paths.tokenizer,
        text_config.dimensions,
    )?;
    info!("Model loaded: {} ({}D)", text_config.model, embedder.dimensions());

    // 6. Create preprocessor
    let preprocessor = create_preprocessor(&text_config.preprocessing.preprocessing_type);
    info!("Preprocessor: {}", preprocessor.name());

    // 7. Create embedding service
    let mut service = EmbeddingService::new(
        pool,
        Box::new(embedder),
        preprocessor,
        config.domain_id.clone(),
        text_config,
    );

    // 8. Run poll loop with graceful shutdown
    let mut interval = tokio::time::interval(Duration::from_secs(config.poll_interval_secs));
    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                match service.run_cycle().await {
                    Ok(summary) => info!("Cycle: {}", summary),
                    Err(e) => error!("Cycle failed: {}", e),
                }
            }
            _ = &mut shutdown => {
                info!("Shutdown signal received");
                break;
            }
        }
    }

    Ok(())
}
```

### `src/service.rs` -- EmbeddingService

```rust
pub struct EmbeddingService {
    pool: Arc<Pool>,
    embedder: Box<dyn TextEmbedder>,
    preprocessor: Box<dyn TextPreprocessor>,
    domain_id: String,
    config: TextEmbeddingConfig,
    last_processed: Option<DateTime<Utc>>,
}

pub struct CycleSummary {
    pub rows_read: usize,
    pub embeddings_stored: usize,
    pub errors: usize,
    pub duration: Duration,
}

impl EmbeddingService {
    pub async fn run_cycle(&mut self) -> Result<CycleSummary> {
        let start = Instant::now();
        let mut summary = CycleSummary::default();

        let client = self.pool.get().await?;

        // 1. Build Gold text view name
        let view_name = format!(
            "gold.{}_text_latest",
            self.domain_id.replace('-', "_")
        );

        // 2. Query for new text rows
        let query = if let Some(last) = self.last_processed {
            format!(
                "SELECT bucket, stream_id, column_name, text_value \
                 FROM {} WHERE bucket > $1 ORDER BY bucket ASC LIMIT 100",
                view_name
            )
        } else {
            format!(
                "SELECT bucket, stream_id, column_name, text_value \
                 FROM {} ORDER BY bucket ASC LIMIT 100",
                view_name
            )
        };

        let rows = match self.execute_query(&client, &query).await {
            Ok(rows) => rows,
            Err(e) if is_relation_not_found(&e) => {
                warn!("Gold text view '{}' not found (dp-023 not implemented?)", view_name);
                return Ok(summary);
            }
            Err(e) => return Err(e),
        };

        // 3. Process each row: preprocess -> embed -> store
        for row in &rows {
            let bucket: DateTime<Utc> = row.get("bucket");
            let stream_id: String = row.get("stream_id");
            let column_name: String = row.get("column_name");
            let text_value: String = row.get("text_value");
            summary.rows_read += 1;

            // Preprocess
            let preprocessed = self.preprocessor.preprocess(&text_value);

            // Embed (single text)
            match self.embedder.embed(&[preprocessed.as_str()]) {
                Ok(vectors) if !vectors.is_empty() => {
                    let vector = &vectors[0];

                    // Store in gold.text_embeddings
                    let insert = "INSERT INTO gold.text_embeddings \
                        (bucket, domain_id, source_stream, source_column, \
                         source_text, embedding, model_id, created_at) \
                        VALUES ($1, $2, $3, $4, $5, $6, $7, now()) \
                        ON CONFLICT DO NOTHING";

                    match client.execute(
                        insert,
                        &[
                            &bucket,
                            &self.domain_id,
                            &stream_id,
                            &column_name,
                            &text_value,
                            &pgvector::Vector::from(vector.clone()),
                            &self.config.model,
                        ],
                    ).await {
                        Ok(_) => summary.embeddings_stored += 1,
                        Err(e) => {
                            warn!("Failed to store embedding: {}", e);
                            summary.errors += 1;
                        }
                    }
                }
                Ok(_) => warn!("Empty embedding result for text"),
                Err(e) => {
                    warn!("Embedding failed: {}", e);
                    summary.errors += 1;
                }
            }

            self.last_processed = Some(bucket);
        }

        summary.duration = start.elapsed();
        Ok(summary)
    }
}

fn is_relation_not_found(err: &anyhow::Error) -> bool {
    // Check for PostgreSQL error code 42P01 (undefined_table)
    err.to_string().contains("42P01")
        || err.to_string().contains("does not exist")
}
```
