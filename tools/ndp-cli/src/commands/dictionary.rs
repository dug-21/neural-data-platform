//! Dictionary subcommand: `ndp dictionary <verb>`.

use clap::{Args, Subcommand};
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

            if dry_run {
                println!(
                    "DRY RUN: Would sync {} stream(s) to data_dictionary",
                    configs.len()
                );
                for config in &configs {
                    println!("  - {} ({})", config.stream_id, config.description);
                }
                return Ok(());
            }

            // Connect to DB and run sync
            // NOTE: Dictionary sync requires StreamConfig -> StreamDictionaryEntry
            // conversion, which is Phase B completion work (not Phase C scope).
            tracing::info!(db_url = %db_url, "Connecting to database");
            let _db = ndp_lib::db::PostgresClient::connect(db_url, 10).await?;
            let _options = ndp_lib::types::SyncOptions { dry_run };

            // TODO(Phase B): Convert Vec<StreamConfig> to Vec<StreamDictionaryEntry>
            //                 then call ndp_lib::dictionary::sync_dictionary()
            println!(
                "dictionary sync: {} stream(s) found (wiring pending Phase B completion)",
                configs.len()
            );

            Ok(())
        }
    }
}
