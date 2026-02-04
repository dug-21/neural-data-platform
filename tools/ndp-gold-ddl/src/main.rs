//! ndp-gold-ddl CLI entry point
//!
//! Gold layer DDL generator for NDP stream configurations.
//!
//! Exit codes:
//! - 0: Generation successful
//! - 1: Generation failed (validation error, config error)
//! - 2: System error (file not found, etc.)

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;
use tracing_subscriber::EnvFilter;

use ndp_gold_ddl::config::{Action, ConfigLoader, FileSystemConfigLoader};
use ndp_gold_ddl::generators::AlignedViewGenerator;

/// Exit codes for the CLI
mod exit_codes {
    pub const SUCCESS: u8 = 0;
    pub const GENERATION_ERROR: u8 = 1;
    pub const SYSTEM_ERROR: u8 = 2;
}

/// ndp-gold-ddl - Gold layer DDL generator for NDP
///
/// Generates TimescaleDB DDL for Gold layer objects including:
/// - Continuous aggregates for individual streams
/// - Aligned materialized views for cross-stream correlation
#[derive(Parser, Debug)]
#[command(name = "ndp-gold-ddl")]
#[command(author = "Neural Data Platform Team")]
#[command(version)]
#[command(about = "Generate Gold layer DDL for NDP streams", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Config directory path
    #[arg(long, env = "NDP_CONFIG_DIR", default_value = "./config")]
    config_dir: PathBuf,

    /// Show verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Generate DDL for a stream or domain
    Generate {
        /// Stream ID for single-stream continuous aggregate
        #[arg(long, conflicts_with = "domain")]
        stream: Option<String>,

        /// Domain ID for cross-stream aligned view
        #[arg(long, conflicts_with = "stream")]
        domain: Option<String>,

        /// Action: sync (idempotent) or recreate (drop and create)
        #[arg(long, default_value = "sync")]
        action: String,
    },

    /// Validate configuration without generating DDL
    Validate {
        /// Stream ID to validate
        #[arg(long, conflicts_with = "domain")]
        stream: Option<String>,

        /// Domain ID to validate
        #[arg(long, conflicts_with = "stream")]
        domain: Option<String>,
    },
}

fn main() -> ExitCode {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();

    match run(&cli) {
        Ok(output) => {
            println!("{}", output);
            ExitCode::from(exit_codes::SUCCESS)
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::from(exit_codes::GENERATION_ERROR)
        }
    }
}

fn run(cli: &Cli) -> Result<String, Box<dyn std::error::Error>> {
    let loader = FileSystemConfigLoader::new(&cli.config_dir);

    match &cli.command {
        Commands::Generate { stream, domain, action } => {
            let action: Action = action.parse().map_err(|e: String| e)?;

            if let Some(domain_id) = domain {
                // Generate aligned view for domain
                let domain_config = loader.load_domain_config(domain_id)?;
                let generator = AlignedViewGenerator::new(loader);
                let sql = generator.generate(&domain_config, action)?;
                Ok(sql)
            } else if let Some(_stream_id) = stream {
                // TODO: Implement continuous aggregate generation
                Err("Stream DDL generation not yet implemented. Use --domain for aligned views.".into())
            } else {
                Err("Must specify --stream or --domain".into())
            }
        }

        Commands::Validate { stream, domain } => {
            if let Some(domain_id) = domain {
                // Validate domain config
                let domain_config = loader.load_domain_config(domain_id)?;

                // Check minimum streams
                if domain_config.streams.len() < 2 {
                    return Err(format!(
                        "Domain '{}' requires at least 2 streams, found {}",
                        domain_id,
                        domain_config.streams.len()
                    )
                    .into());
                }

                // Check for primary stream
                let has_primary = domain_config
                    .streams
                    .iter()
                    .any(|s| s.role == ndp_gold_ddl::StreamRole::Primary);

                if !has_primary {
                    return Err(format!(
                        "Domain '{}' requires a stream with role 'primary'",
                        domain_id
                    )
                    .into());
                }

                // Validate each stream exists
                for stream_ref in &domain_config.streams {
                    loader.load_stream_config(&stream_ref.stream_id)?;
                }

                Ok(format!("Domain '{}' configuration is valid", domain_id))
            } else if let Some(stream_id) = stream {
                // Validate stream config
                let stream_config = loader.load_stream_config(stream_id)?;

                if stream_config.gold_etl.is_none() {
                    return Err(format!(
                        "Stream '{}' has no gold_etl configuration",
                        stream_id
                    )
                    .into());
                }

                let gold_etl = stream_config.gold_etl.unwrap();
                if !gold_etl.enabled {
                    return Err(format!(
                        "Stream '{}' has gold_etl.enabled = false",
                        stream_id
                    )
                    .into());
                }

                Ok(format!("Stream '{}' Gold ETL configuration is valid", stream_id))
            } else {
                Err("Must specify --stream or --domain".into())
            }
        }
    }
}
