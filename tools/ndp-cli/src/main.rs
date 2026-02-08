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

    /// Database connection timeout in seconds.
    #[arg(long, default_value = "10", global = true)]
    db_timeout: u64,

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

    /// Domain configuration operations.
    Domain(commands::domain::DomainArgs),

    /// Gold layer DDL operations.
    Gold(commands::gold::GoldArgs),

    /// Validate stream and domain configurations.
    Validate(commands::validate::ValidateArgs),
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
}

/// Require a database URL, exiting with an error if missing.
///
/// Called for commands that always need a database connection (dictionary,
/// dimension, domain). Gold commands handle the optional URL internally
/// since `generate` does not require a database.
fn require_db_url(db_url: &Option<String>) -> String {
    db_url.clone().unwrap_or_else(|| {
        eprintln!("Error: No database URL provided.");
        eprintln!("Pass --db-url or set TIMESCALE_URL environment variable.");
        std::process::exit(1);
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing (default to warn; use RUST_LOG=info for verbose)
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();
    let config_dir = cli.resolve_config_dir();
    let db_url = cli.db_url;
    let db_timeout = cli.db_timeout;

    match cli.command {
        Commands::Dictionary(args) => {
            let db_url = require_db_url(&db_url);
            commands::dictionary::run(args, &config_dir, &db_url).await?;
        }
        Commands::Dimension(args) => {
            let db_url = require_db_url(&db_url);
            commands::dimension::run(args, &config_dir, &db_url).await?;
        }
        Commands::Domain(args) => {
            let db_url = require_db_url(&db_url);
            commands::domain::run(args, &config_dir, &db_url).await?;
        }
        Commands::Gold(args) => {
            commands::gold::run(args, &config_dir, db_url.as_deref(), db_timeout).await?;
        }
        Commands::Validate(args) => {
            let exit_code = commands::validate::run(args, &config_dir);
            std::process::exit(exit_code);
        }
    }

    Ok(())
}
