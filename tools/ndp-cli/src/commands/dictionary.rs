//! Dictionary subcommand: `ndp dictionary <verb>`.

use clap::{Args, Subcommand};
use ndp_lib::NoOpDbClient;
use std::path::Path;

/// Data dictionary operations.
#[derive(Args)]
pub struct DictionaryArgs {
    #[command(subcommand)]
    pub command: DictionaryCommands,
}

#[derive(Subcommand)]
pub enum DictionaryCommands {
    /// Sync stream configs to the data_dictionary tables.
    Sync {
        /// Config directory containing stream subdirectories.
        /// Defaults to <config-base>/streams.
        #[arg(long)]
        config_dir: Option<std::path::PathBuf>,

        /// Print SQL without executing.
        #[arg(long)]
        dry_run: bool,
    },
}

/// Run the dictionary subcommand.
pub async fn run(
    args: DictionaryArgs,
    base_config_dir: &Path,
    db_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match args.command {
        DictionaryCommands::Sync {
            config_dir,
            dry_run,
        } => {
            let streams_dir = config_dir.unwrap_or_else(|| base_config_dir.join("streams"));

            tracing::info!(
                streams_dir = %streams_dir.display(),
                db_url = %db_url,
                dry_run = dry_run,
                "Starting dictionary sync"
            );

            // Load stream configs via FileSystemConfigLoader
            let loader = ndp_lib::config::FileSystemConfigLoader::new(
                &streams_dir,
                base_config_dir.join("dimensions"),
            );

            let configs = ndp_lib::config::ConfigLoader::load_stream_configs(&loader)?;

            tracing::info!(stream_count = configs.len(), "Loaded stream configurations");

            // Convert StreamConfig -> StreamDictionaryEntry
            let entries: Vec<ndp_lib::dictionary::types::StreamDictionaryEntry> = configs
                .iter()
                .map(ndp_lib::convert::stream_config_to_dictionary_entry)
                .collect();

            let options = ndp_lib::types::SyncOptions {
                dry_run,
                ..Default::default()
            };

            if dry_run {
                let report =
                    ndp_lib::dictionary::sync_dictionary(&entries, &NoOpDbClient, &options).await?;

                println!("DRY RUN dictionary sync:");
                println!("  Streams:        {}", report.items_processed);
                println!("  Bronze items:   {}", report.items_created);
                println!("  Silver items:   {}", report.items_updated);

                for config in &configs {
                    let etl_info = config
                        .silver_etl
                        .as_ref()
                        .and_then(|e| e.target_table.as_deref())
                        .unwrap_or("(no silver)");
                    println!("  - {} -> {}", config.stream_id, etl_info);
                }
                return Ok(());
            }

            // Connect to DB and run sync
            tracing::info!(db_url = %db_url, "Connecting to database");
            let db = ndp_lib::db::PostgresClient::connect(db_url, 10).await?;

            let report = ndp_lib::dictionary::sync_dictionary(&entries, &db, &options).await?;

            println!("Dictionary sync complete:");
            println!("  Streams synced: {}", report.items_processed);
            println!("  Bronze created: {}", report.items_created);
            println!("  Silver updated: {}", report.items_updated);
            println!("  Duration:       {:.2}s", report.duration.as_secs_f64());

            if !report.errors.is_empty() {
                println!("  Warnings:       {}", report.errors.len());
                for err in &report.errors {
                    println!("    - {}: {}", err.item, err.message);
                }
            }

            Ok(())
        }
    }
}
