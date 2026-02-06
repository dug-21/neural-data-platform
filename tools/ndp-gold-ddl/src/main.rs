//! ndp-gold-ddl CLI entry point
//!
//! Gold layer DDL generator for NDP stream configurations.
//!
//! Exit codes:
//! - 0: Generation successful
//! - 1: Generation failed (validation error, config error)
//! - 2: System error (file not found, etc.)
//! - 3: Database connection error

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;
use tracing_subscriber::EnvFilter;

use ndp_gold_ddl::config::{Action, ConfigLoader, FileSystemConfigLoader};
use ndp_gold_ddl::db::{PostgresCaChecker, PostgresClient};
use ndp_gold_ddl::generators::{
    AlignedViewGenerator, ContinuousAggregateGenerator, EventsGenerator,
    StateTransitionGenerator, TransitionConfig,
};
use ndp_gold_ddl::planner::SyncPlanner;

/// Exit codes for the CLI
mod exit_codes {
    pub const SUCCESS: u8 = 0;
    pub const GENERATION_ERROR: u8 = 1;
    #[allow(dead_code)]
    pub const SYSTEM_ERROR: u8 = 2;
    pub const DATABASE_ERROR: u8 = 3;
}

/// ndp-gold-ddl - Gold layer DDL generator for NDP
///
/// Generates TimescaleDB DDL for Gold layer objects including:
/// - Continuous aggregates for individual streams
/// - Aligned materialized views for cross-stream correlation
///
/// ## Sync Mode with Database
///
/// When --database-url is provided, the tool checks which continuous aggregates
/// already exist and only generates DDL for missing ones. This provides true
/// idempotency without complex Bash parsing.
///
/// ## Dry-Run Mode
///
/// When --database-url is omitted, the tool generates all DDL. Useful for
/// previewing changes or when database connectivity isn't available.
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

    /// Database URL for existence checks (enables intelligent sync)
    ///
    /// When provided, the tool connects to TimescaleDB to check which
    /// continuous aggregates exist and only generates DDL for missing ones.
    ///
    /// Format: postgresql://user:pass@host:port/dbname
    #[arg(long, env = "TIMESCALE_URL")]
    database_url: Option<String>,

    /// Database connection timeout in seconds
    #[arg(long, default_value = "10")]
    db_timeout: u64,

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

        /// Generate state transitions view instead of continuous aggregate
        #[arg(long)]
        transitions: bool,

        /// Generate events infrastructure DDL (requires --domain)
        #[arg(long)]
        events: bool,
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

#[tokio::main]
async fn main() -> ExitCode {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();

    match run(&cli).await {
        Ok(output) => {
            println!("{}", output);
            ExitCode::from(exit_codes::SUCCESS)
        }
        Err(e) => {
            let exit_code =
                if e.to_string().contains("Database") || e.to_string().contains("Connection") {
                    exit_codes::DATABASE_ERROR
                } else {
                    exit_codes::GENERATION_ERROR
                };
            eprintln!("Error: {}", e);
            ExitCode::from(exit_code)
        }
    }
}

async fn run(cli: &Cli) -> Result<String, Box<dyn std::error::Error>> {
    let loader = FileSystemConfigLoader::new(&cli.config_dir);

    match &cli.command {
        Commands::Generate {
            stream,
            domain,
            action,
            transitions,
            events,
        } => {
            let action: Action = action.parse().map_err(|e: String| e)?;

            if let Some(domain_id) = domain {
                if *events {
                    // Generate events infrastructure DDL for domain
                    let domain_config = loader.load_domain_config(domain_id)?;
                    let generator = EventsGenerator::from_domain_config(&domain_config);
                    let sql = generator.generate(action)?;
                    Ok(sql)
                } else {
                    // Generate aligned view for domain
                    // Note: Domain generation doesn't use DB checks yet (future enhancement)
                    let domain_config = loader.load_domain_config(domain_id)?;
                    let generator = AlignedViewGenerator::new(loader);
                    let sql = generator.generate(&domain_config, action)?;
                    Ok(sql)
                }
            } else if *events {
                Err("--events requires --domain".into())
            } else if let Some(stream_id) = stream {
                // Generate DDL for stream
                let stream_config = loader.load_stream_config(stream_id)?;
                let gold_etl = stream_config.gold_etl.as_ref().ok_or_else(|| {
                    format!("Stream '{}' has no gold_etl configuration", stream_id)
                })?;

                if !gold_etl.enabled {
                    return Err(
                        format!("Stream '{}' has gold_etl.enabled = false", stream_id).into(),
                    );
                }

                // Check if transitions are requested
                if *transitions {
                    // Generate state transitions view
                    generate_transitions(cli, &stream_config, action)
                } else if let (Some(db_url), Action::Sync) = (&cli.database_url, action) {
                    // If database URL is provided and action is sync, use the planner
                    generate_with_db_check(cli, &stream_config, gold_etl, db_url).await
                } else {
                    // No DB URL or recreate mode - generate all DDL
                    if cli.verbose {
                        eprintln!(
                            "Note: No database URL provided, generating all DDL (dry-run mode)"
                        );
                    }
                    let generator =
                        ContinuousAggregateGenerator::from_stream_config(&stream_config)?;
                    let sql = generator.generate(gold_etl, action)?;
                    Ok(sql)
                }
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
                    return Err(
                        format!("Stream '{}' has no gold_etl configuration", stream_id).into(),
                    );
                }

                let gold_etl = stream_config.gold_etl.unwrap();
                if !gold_etl.enabled {
                    return Err(
                        format!("Stream '{}' has gold_etl.enabled = false", stream_id).into(),
                    );
                }

                Ok(format!(
                    "Stream '{}' Gold ETL configuration is valid",
                    stream_id
                ))
            } else {
                Err("Must specify --stream or --domain".into())
            }
        }
    }
}

/// Generate DDL with database existence checking
async fn generate_with_db_check(
    cli: &Cli,
    stream_config: &ndp_gold_ddl::StreamConfig,
    gold_etl: &ndp_gold_ddl::GoldEtlConfig,
    db_url: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    if cli.verbose {
        eprintln!("Connecting to database for existence checks...");
    }

    // Connect to database
    let client = PostgresClient::connect(db_url, cli.db_timeout).await?;
    let checker = PostgresCaChecker::new(client);

    // Create sync plan
    let planner = SyncPlanner::new(&checker, stream_config);
    let plan = planner.plan(gold_etl).await?;

    if cli.verbose {
        eprintln!("{}", plan.summary());
    }

    // Generate DDL from plan
    Ok(plan.to_ddl())
}

/// Generate state transitions DDL for a stream
fn generate_transitions(
    cli: &Cli,
    stream_config: &ndp_gold_ddl::StreamConfig,
    action: Action,
) -> Result<String, Box<dyn std::error::Error>> {
    // Get transition config from stream's gold_etl.features.transitions section
    let transition_config =
        TransitionConfig::from_stream_config(stream_config).unwrap_or_else(|| {
            // Default config if not specified in stream config
            TransitionConfig::new("state", "ndp_id")
        });

    if cli.verbose {
        eprintln!(
            "Generating state transitions for stream '{}' (state_field: {}, entity_field: {})",
            stream_config.stream_id, transition_config.state_field, transition_config.entity_field
        );
    }

    let generator = StateTransitionGenerator::from_stream_config(stream_config)?;
    let sql = generator.generate(&transition_config, action)?;
    Ok(sql)
}
