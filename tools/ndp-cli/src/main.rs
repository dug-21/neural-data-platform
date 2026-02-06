//! NDP CLI - Neural Data Platform deployment tool.
//!
//! Entity/verb command structure: `ndp <entity> <verb> [args]`.
//!
//! # Examples
//!
//! ```bash
//! ndp dictionary sync --config-dir config/base/streams
//! ndp dictionary sync --config-dir config/base/streams --dry-run
//! ndp dimension sync entity_context --source data.csv
//! ```

use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod commands;

#[derive(Parser)]
#[command(
    name = "ndp",
    about = "Neural Data Platform CLI",
    version,
    propagate_version = true
)]
struct Cli {
    /// Database URL (or set TIMESCALE_URL env var).
    #[arg(long, env = "TIMESCALE_URL", global = true)]
    db_url: Option<String>,

    /// Environment: integration or pi.
    #[arg(long, env = "DEPLOY_ENV", default_value = "pi", global = true)]
    env: String,

    /// Config base directory (defaults based on --env).
    #[arg(long, global = true)]
    config_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Data dictionary operations.
    Dictionary(commands::dictionary::DictionaryArgs),

    /// Dimension table operations.
    Dimension(commands::dimension::DimensionArgs),
}

impl Cli {
    /// Resolve the config base directory based on --config-dir or --env.
    ///
    /// Returns the base directory containing `streams/` and `dimensions/` subdirectories.
    fn resolve_config_dir(&self) -> PathBuf {
        self.config_dir
            .clone()
            .unwrap_or_else(|| match self.env.as_str() {
                "integration" => PathBuf::from("config/integration/base"),
                _ => PathBuf::from("config/base"),
            })
    }

    /// Resolve the database URL based on --db-url or --env.
    fn resolve_db_url(&self) -> String {
        self.db_url
            .clone()
            .unwrap_or_else(|| match self.env.as_str() {
                "integration" => "postgresql://postgres:postgres@localhost:5432/ndp".into(),
                _ => "postgresql://postgres:postgres@timescaledb:5432/ndp".into(),
            })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let config_dir = cli.resolve_config_dir();
    let db_url = cli.resolve_db_url();

    match cli.command {
        Commands::Dictionary(args) => {
            commands::dictionary::run(args, &config_dir, &db_url).await?;
        }
        Commands::Dimension(args) => {
            commands::dimension::run(args, &config_dir, &db_url).await?;
        }
    }

    Ok(())
}
