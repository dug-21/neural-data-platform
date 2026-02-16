//! NDP Intelligence Application
//!
//! Binary for intelligence operations: daemon mode (continuous intelligence cycle),
//! one-shot mode (single cycle), backfill mode (historical embedding generation),
//! and status reporting.

mod config;

use std::sync::Arc;
use std::time::Duration;

use clap::{Parser, Subcommand};
use tracing::{error, info, warn};

use config::load_intelligence_config;
use ndp_intelligence::service::{AppConfig, IntelligenceService};

#[derive(Parser)]
#[command(name = "ndp-intelligence")]
#[command(about = "NDP Intelligence - similarity search and prediction daemon")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the intelligence daemon (timer + NOTIFY wake, continuous cycle)
    Daemon,
    /// Run a one-shot intelligence cycle (single pass, exit 0)
    OneShot {
        /// Domain ID to process
        #[arg(long)]
        domain: Option<String>,
    },
    /// Backfill embeddings from historical Gold data (embed-only, no predictions)
    Backfill {
        /// Domain ID to backfill
        #[arg(long)]
        domain: Option<String>,
        /// Start timestamp (ISO 8601, e.g., 2026-01-01T00:00:00Z)
        #[arg(long)]
        since: Option<String>,
    },
    /// Show intelligence system status
    Status,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Daemon => run_daemon().await,
        Commands::OneShot { domain } => run_one_shot(domain).await,
        Commands::Backfill { domain, since } => run_backfill(domain, since).await,
        Commands::Status => run_status().await,
    };

    if let Err(e) = result {
        error!("Fatal error: {}", e);
        std::process::exit(1);
    }
}

/// Create a deadpool-postgres connection pool.
fn create_pool(
    app_config: &AppConfig,
) -> Result<Arc<deadpool_postgres::Pool>, ndp_intelligence::error::IntelligenceError> {
    let pg_config: tokio_postgres::Config =
        app_config
            .database_url
            .parse()
            .map_err(|e: tokio_postgres::Error| {
                ndp_intelligence::error::IntelligenceError::Config {
                    message: format!("Invalid DATABASE_URL: {}", e),
                }
            })?;

    let mgr_config = deadpool_postgres::ManagerConfig {
        recycling_method: deadpool_postgres::RecyclingMethod::Fast,
    };
    let mgr = deadpool_postgres::Manager::from_config(pg_config, tokio_postgres::NoTls, mgr_config);
    let pool = deadpool_postgres::Pool::builder(mgr)
        .max_size(app_config.pool_size)
        .build()
        .map_err(|e| ndp_intelligence::error::IntelligenceError::Config {
            message: format!("Failed to create connection pool: {}", e),
        })?;

    Ok(Arc::new(pool))
}

/// Create a direct PostgreSQL client for StorageBackend.
///
/// StorageBackend uses Arc<Client> (single connection), while the pool is used
/// by PredictionEngine, OutcomeTracker, and IntelligenceService for queries.
async fn create_storage_client(
    database_url: &str,
) -> Result<Arc<tokio_postgres::Client>, ndp_intelligence::error::IntelligenceError> {
    let (client, connection) = tokio_postgres::connect(database_url, tokio_postgres::NoTls)
        .await
        .map_err(|e| {
            ndp_intelligence::error::IntelligenceError::Database(format!(
                "Failed to connect to PostgreSQL: {}",
                e
            ))
        })?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            tracing::error!("Storage connection error: {}", e);
        }
    });

    Ok(Arc::new(client))
}

/// Run the intelligence daemon with timer + NOTIFY wake.
async fn run_daemon() -> Result<(), ndp_intelligence::error::IntelligenceError> {
    let app_config = AppConfig::from_env()?;
    let pool = create_pool(&app_config)?;

    // Load domain config from etcd via config-client
    let (intel_config, objectives, primary_alias) = load_intelligence_config(&app_config).await?;

    // Create storage backend (dedicated connection, not pooled)
    let storage_client = create_storage_client(&app_config.database_url).await?;
    let storage = Arc::new(ndp_intelligence::storage::postgres::PostgresStorage::new(
        storage_client,
    ));

    // Create intelligence service
    let mut service = IntelligenceService::new(
        &app_config,
        &intel_config,
        objectives,
        pool.clone(),
        storage,
        primary_alias,
    )
    .await?;

    // Start NOTIFY listener (optional, non-fatal if fails)
    let notify_listener =
        ndp_intelligence::notify::NotifyListener::new(&app_config.database_url, "gold_refresh");
    let mut notify_rx = match notify_listener.listen().await {
        Ok(rx) => {
            info!("PG NOTIFY listener started");
            Some(rx)
        }
        Err(e) => {
            warn!(
                "PG NOTIFY listener failed to start: {}. Using timer only.",
                e
            );
            None
        }
    };

    // Timer fallback (primary wake mechanism)
    let mut timer = tokio::time::interval(Duration::from_secs(app_config.poll_interval_secs));
    timer.tick().await; // first tick fires immediately

    info!(
        "Intelligence daemon started for domain '{}' (poll interval: {}s, warmup: {})",
        app_config.domain_id, app_config.poll_interval_secs, app_config.warmup_threshold
    );

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Shutdown signal received");
                break;
            }
            Some(payload) = async {
                match notify_rx.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                info!("PG NOTIFY received: {}", payload);
                match service.run_cycle().await {
                    Ok(summary) => info!("Cycle (NOTIFY): {}", summary),
                    Err(e) => error!("Cycle failed: {}", e),
                }
            }
            _ = timer.tick() => {
                match service.run_cycle().await {
                    Ok(summary) => info!("Cycle (timer): {}", summary),
                    Err(e) => error!("Cycle failed: {}", e),
                }
            }
        }
    }

    info!("Intelligence daemon stopped");
    Ok(())
}

/// Run a single intelligence cycle and exit.
async fn run_one_shot(
    domain_override: Option<String>,
) -> Result<(), ndp_intelligence::error::IntelligenceError> {
    let mut app_config = AppConfig::from_env()?;
    if let Some(domain) = domain_override {
        app_config.domain_id = domain;
    }

    let pool = create_pool(&app_config)?;
    let (intel_config, objectives, primary_alias) = load_intelligence_config(&app_config).await?;

    let storage_client = create_storage_client(&app_config.database_url).await?;
    let storage = Arc::new(ndp_intelligence::storage::postgres::PostgresStorage::new(
        storage_client,
    ));

    let mut service = IntelligenceService::new(
        &app_config,
        &intel_config,
        objectives,
        pool,
        storage,
        primary_alias,
    )
    .await?;

    let summary = service.run_cycle().await?;
    info!("One-shot cycle complete: {}", summary);
    Ok(())
}

/// Run backfill mode: embed historical Gold data without generating predictions.
async fn run_backfill(
    domain_override: Option<String>,
    since: Option<String>,
) -> Result<(), ndp_intelligence::error::IntelligenceError> {
    let mut app_config = AppConfig::from_env()?;
    if let Some(domain) = domain_override {
        app_config.domain_id = domain;
    }

    let pool = create_pool(&app_config)?;
    let (intel_config, objectives, primary_alias) = load_intelligence_config(&app_config).await?;

    let storage_client = create_storage_client(&app_config.database_url).await?;
    let storage = Arc::new(ndp_intelligence::storage::postgres::PostgresStorage::new(
        storage_client,
    ));

    let mut service = IntelligenceService::new(
        &app_config,
        &intel_config,
        objectives,
        pool,
        storage,
        primary_alias,
    )
    .await?;
    service.set_backfill_mode(true);

    if let Some(since_str) = since {
        let since_dt = since_str
            .parse::<chrono::DateTime<chrono::Utc>>()
            .map_err(|e| ndp_intelligence::error::IntelligenceError::Config {
                message: format!("Invalid --since timestamp '{}': {}", since_str, e),
            })?;
        service.set_last_processed(Some(since_dt - chrono::Duration::seconds(1)));
    }

    let mut total_embeddings = 0usize;
    loop {
        let summary = service.run_cycle().await?;
        total_embeddings += summary.embeddings_generated;
        if summary.rows_observed == 0 {
            break; // no more rows to process
        }
    }

    info!(
        "Backfill complete: {} total embeddings generated for domain '{}'",
        total_embeddings, app_config.domain_id
    );
    Ok(())
}

/// Show intelligence system status.
async fn run_status() -> Result<(), ndp_intelligence::error::IntelligenceError> {
    let app_config = AppConfig::from_env()?;
    let client = create_storage_client(&app_config.database_url).await?;

    // Query embedding count
    let emb_count = client
        .query_one(
            "SELECT count(*)::bigint FROM gold.metric_embeddings WHERE domain_id = $1",
            &[&app_config.domain_id],
        )
        .await
        .map_err(|e| {
            ndp_intelligence::error::IntelligenceError::Database(format!("Query error: {}", e))
        })?;
    let embedding_count: i64 = emb_count.get(0);

    // Query prediction stats
    let pred_stats = client
        .query_one(
            "SELECT count(*)::bigint AS total,
                    count(actual_value)::bigint AS evaluated,
                    count(CASE WHEN correct = true THEN 1 END)::bigint AS correct
             FROM gold.predictions
             WHERE domain_id = $1",
            &[&app_config.domain_id],
        )
        .await
        .map_err(|e| {
            ndp_intelligence::error::IntelligenceError::Database(format!("Query error: {}", e))
        })?;

    let total_preds: i64 = pred_stats.get(0);
    let evaluated: i64 = pred_stats.get(1);
    let correct: i64 = pred_stats.get(2);

    println!("Intelligence Status for domain '{}':", app_config.domain_id);
    println!("  Embeddings: {}", embedding_count);
    println!(
        "  Warmed up: {}",
        embedding_count >= app_config.warmup_threshold as i64
    );
    println!(
        "  Predictions: {} total, {} evaluated",
        total_preds, evaluated
    );
    if evaluated > 0 {
        let accuracy = correct as f64 / evaluated as f64 * 100.0;
        println!("  Accuracy: {:.1}% ({}/{})", accuracy, correct, evaluated);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_cli_parses_daemon() {
        let cli = Cli::try_parse_from(["ndp-intelligence", "daemon"]);
        assert!(cli.is_ok(), "Should parse daemon subcommand");
        assert!(matches!(cli.unwrap().command, Commands::Daemon));
    }

    #[test]
    fn test_cli_parses_one_shot() {
        let cli = Cli::try_parse_from(["ndp-intelligence", "one-shot", "--domain", "test"]);
        assert!(cli.is_ok(), "Should parse one-shot subcommand");
        match cli.unwrap().command {
            Commands::OneShot { domain } => assert_eq!(domain, Some("test".to_string())),
            _ => panic!("Expected OneShot"),
        }
    }

    #[test]
    fn test_cli_parses_one_shot_no_domain() {
        let cli = Cli::try_parse_from(["ndp-intelligence", "one-shot"]);
        assert!(cli.is_ok(), "Should parse one-shot without --domain");
        match cli.unwrap().command {
            Commands::OneShot { domain } => assert!(domain.is_none()),
            _ => panic!("Expected OneShot"),
        }
    }

    #[test]
    fn test_cli_parses_backfill() {
        let cli = Cli::try_parse_from(["ndp-intelligence", "backfill", "--domain", "test"]);
        assert!(cli.is_ok(), "Should parse backfill subcommand");
        match cli.unwrap().command {
            Commands::Backfill { domain, since } => {
                assert_eq!(domain, Some("test".to_string()));
                assert!(since.is_none());
            }
            _ => panic!("Expected Backfill"),
        }
    }

    #[test]
    fn test_cli_parses_backfill_with_since() {
        let cli = Cli::try_parse_from([
            "ndp-intelligence",
            "backfill",
            "--domain",
            "test",
            "--since",
            "2026-01-01T00:00:00Z",
        ]);
        assert!(cli.is_ok(), "Should parse backfill with --since");
        match cli.unwrap().command {
            Commands::Backfill { domain, since } => {
                assert_eq!(domain, Some("test".to_string()));
                assert_eq!(since.unwrap(), "2026-01-01T00:00:00Z");
            }
            _ => panic!("Expected Backfill"),
        }
    }

    #[test]
    fn test_cli_parses_status() {
        let cli = Cli::try_parse_from(["ndp-intelligence", "status"]);
        assert!(cli.is_ok(), "Should parse status subcommand");
        assert!(matches!(cli.unwrap().command, Commands::Status));
    }

    #[test]
    fn test_cli_requires_subcommand() {
        let cli = Cli::try_parse_from(["ndp-intelligence"]);
        assert!(cli.is_err(), "Should require a subcommand");
    }
}
