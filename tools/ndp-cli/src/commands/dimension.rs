//! Dimension subcommand: `ndp dimension <verb>`.

use async_trait::async_trait;
use clap::{Args, Subcommand};
use std::path::Path;

/// Dimension table operations.
#[derive(Args)]
pub struct DimensionArgs {
    #[command(subcommand)]
    pub command: DimensionCommands,
}

#[derive(Subcommand)]
pub enum DimensionCommands {
    /// Sync a dimension table from source data (CSV).
    Sync {
        /// Dimension ID (e.g. entity_context).
        dimension_id: String,

        /// Path to the dimension config file.
        #[arg(long)]
        config: Option<std::path::PathBuf>,

        /// Path to the source data file (CSV).
        #[arg(long)]
        source: Option<std::path::PathBuf>,

        /// Print SQL without executing.
        #[arg(long)]
        dry_run: bool,
    },
}

/// Run the dimension subcommand.
pub async fn run(
    args: DimensionArgs,
    base_config_dir: &Path,
    db_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match args.command {
        DimensionCommands::Sync {
            dimension_id,
            config,
            source,
            dry_run,
        } => {
            // Resolve config path: explicit --config, or convention-based lookup
            let config_path = config.unwrap_or_else(|| {
                base_config_dir
                    .join("dimensions")
                    .join(&dimension_id)
                    .join("config.json")
            });

            tracing::info!(
                dimension_id = %dimension_id,
                config_path = %config_path.display(),
                db_url = %db_url,
                dry_run = dry_run,
                "Starting dimension sync"
            );

            // Load dimension config
            let config_content = std::fs::read_to_string(&config_path).map_err(|e| {
                format!(
                    "Failed to read dimension config {}: {}",
                    config_path.display(),
                    e
                )
            })?;
            let dim_config: ndp_lib::config::DimensionConfig =
                serde_json::from_str(&config_content).map_err(|e| {
                    format!(
                        "Failed to parse dimension config {}: {}",
                        config_path.display(),
                        e
                    )
                })?;

            // Resolve source path: explicit --source, config's source.path, or convention
            let source_path = source.unwrap_or_else(|| {
                if let Some(ref p) = dim_config.source.path {
                    // If relative, resolve against repo root (parent of config base)
                    let path = std::path::PathBuf::from(p);
                    if path.is_relative() {
                        base_config_dir
                            .parent()
                            .unwrap_or(Path::new("."))
                            .parent()
                            .unwrap_or(Path::new("."))
                            .join(p)
                    } else {
                        path
                    }
                } else {
                    base_config_dir
                        .parent()
                        .unwrap_or(Path::new("."))
                        .join("data")
                        .join("dimensions")
                        .join(format!("{}.csv", dimension_id))
                }
            });

            tracing::info!(source_path = %source_path.display(), "Resolved source path");

            // Read CSV source data
            let csv_content = std::fs::read(&source_path).map_err(|e| {
                format!(
                    "Failed to read source file {}: {}",
                    source_path.display(),
                    e
                )
            })?;

            let options = ndp_lib::types::SyncOptions { dry_run };

            if dry_run {
                // Dry run: parse CSV and show what would happen, without DB.
                // Use a no-op DB client since sync_dimension skips execution.
                let report = ndp_lib::dimension::sync_dimension(
                    &dim_config,
                    &csv_content,
                    &NoOpDbClient,
                    &options,
                )
                .await?;

                println!("DRY RUN dimension sync:");
                println!("  Dimension:    {}", dimension_id);
                println!(
                    "  Target:       {}.{}",
                    dim_config.target.schema, dim_config.target.table
                );
                let strategy = dim_config
                    .load
                    .as_ref()
                    .map(|l| l.strategy.as_str())
                    .unwrap_or("truncate_and_load");
                println!("  Strategy:     {}", strategy);
                println!("  Rows:         {}", report.items_processed);
            } else {
                // Connect to DB and run sync
                tracing::info!(db_url = %db_url, "Connecting to database");
                let db = ndp_lib::db::PostgresClient::connect(db_url, 10).await?;

                let report =
                    ndp_lib::dimension::sync_dimension(&dim_config, &csv_content, &db, &options)
                        .await?;

                println!("Dimension sync complete:");
                println!("  Dimension:      {}", dimension_id);
                println!("  Rows processed: {}", report.items_processed);
                println!("  Created:        {}", report.items_created);
                println!("  Updated:        {}", report.items_updated);
                println!("  Deleted:        {}", report.items_deleted);
                println!("  Duration:       {:.2}s", report.duration.as_secs_f64());

                if !report.errors.is_empty() {
                    println!("  Warnings:       {}", report.errors.len());
                    for err in &report.errors {
                        println!("    - {}: {}", err.item, err.message);
                    }
                }
            }

            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// NoOpDbClient for dry-run mode
// ---------------------------------------------------------------------------

/// A `DbClient` that is never invoked. Used for dry_run mode where
/// `sync_dimension` skips all SQL execution internally.
struct NoOpDbClient;

#[async_trait]
impl ndp_lib::DbClient for NoOpDbClient {
    async fn query(
        &self,
        _query: &str,
        _params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> ndp_lib::Result<Vec<tokio_postgres::Row>> {
        unreachable!("NoOpDbClient should not be called in dry_run mode")
    }

    async fn execute(
        &self,
        _query: &str,
        _params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> ndp_lib::Result<u64> {
        unreachable!("NoOpDbClient should not be called in dry_run mode")
    }

    async fn batch_execute(&self, _sql: &str) -> ndp_lib::Result<()> {
        unreachable!("NoOpDbClient should not be called in dry_run mode")
    }
}
