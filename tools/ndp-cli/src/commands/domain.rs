//! Domain subcommand: `ndp domain <verb>`.

use clap::{Args, Subcommand};
use std::path::Path;

/// Domain configuration operations.
#[derive(Args)]
pub struct DomainArgs {
    #[command(subcommand)]
    pub command: DomainCommands,
}

#[derive(Subcommand)]
pub enum DomainCommands {
    /// Sync domain configs to the data_dictionary tables.
    Sync {
        /// Domain config directory (contains domain subdirectories with domain.json).
        /// Defaults to <config-dir>/../domains.
        #[arg(long)]
        domains_dir: Option<std::path::PathBuf>,

        /// Print what would be synced without executing.
        #[arg(long)]
        dry_run: bool,
    },
}

/// Run the domain subcommand.
pub async fn run(
    args: DomainArgs,
    base_config_dir: &Path,
    db_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match args.command {
        DomainCommands::Sync {
            domains_dir,
            dry_run,
        } => {
            // Resolve domains directory
            // Domains are at config/domains, NOT under config/base/
            // base_config_dir points to config/base, so go up one level
            let domains_dir = domains_dir.unwrap_or_else(|| {
                base_config_dir
                    .parent()
                    .unwrap_or(base_config_dir)
                    .join("domains")
            });

            tracing::info!(
                domains_dir = %domains_dir.display(),
                db_url = %db_url,
                dry_run = dry_run,
                "Starting domain sync"
            );

            // Load domain configs via FileSystemConfigLoader
            let loader = ndp_lib::config::FileSystemConfigLoader::new(
                base_config_dir.join("streams"),
                base_config_dir.join("dimensions"),
            )
            .with_domains_dir(&domains_dir);

            let configs = ndp_lib::config::ConfigLoader::load_domain_configs(&loader)?;

            tracing::info!(domain_count = configs.len(), "Loaded domain configurations");

            if configs.is_empty() {
                println!(
                    "No domain configs found in {}. Nothing to sync.",
                    domains_dir.display()
                );
                return Ok(());
            }

            // Convert DomainConfig -> DomainSyncEntry
            let entries: Vec<ndp_lib::domain::types::DomainSyncEntry> = configs
                .iter()
                .map(ndp_lib::convert::domain_config_to_sync_entry)
                .collect();

            let options = ndp_lib::types::SyncOptions { dry_run };

            if dry_run {
                let report =
                    ndp_lib::domain::sync_domains(&entries, &NoOpDbClient, &options).await?;

                println!("DRY RUN domain sync:");
                println!("  Domains:     {}", report.items_processed);
                println!("  Total items: {}", report.items_created);

                for config in &configs {
                    println!(
                        "  - {} ({} streams, {} objectives, {} constraints)",
                        config.id,
                        config.streams.len(),
                        config.objectives.len(),
                        config.constraints.len(),
                    );
                }
                return Ok(());
            }

            // Connect to DB and run sync
            tracing::info!(db_url = %db_url, "Connecting to database");
            let db = ndp_lib::db::PostgresClient::connect(db_url, 10).await?;

            let report = ndp_lib::domain::sync_domains(&entries, &db, &options).await?;

            println!("Domain sync complete:");
            println!("  Domains synced:     {}", report.items_processed);
            println!(
                "  Streams mapped:     {}",
                entries.iter().map(|e| e.streams.len()).sum::<usize>()
            );
            println!(
                "  Objectives synced:  {}",
                entries.iter().map(|e| e.objectives.len()).sum::<usize>()
            );
            println!(
                "  Constraints synced: {}",
                entries.iter().map(|e| e.constraints.len()).sum::<usize>()
            );
            println!(
                "  Duration:           {:.2}s",
                report.duration.as_secs_f64()
            );

            if !report.errors.is_empty() {
                println!("  Warnings:           {}", report.errors.len());
                for err in &report.errors {
                    println!("    - {}: {}", err.item, err.message);
                }
            }

            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// NoOpDbClient for dry-run mode
// ---------------------------------------------------------------------------

use async_trait::async_trait;

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
