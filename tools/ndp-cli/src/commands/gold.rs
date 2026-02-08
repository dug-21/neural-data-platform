//! Gold subcommand: `ndp gold <verb>`.
//!
//! Routes Gold DDL operations to `ndp_lib::gold::*` functions.
//!
//! # Subcommands
//!
//! - `generate` - Generate Gold DDL without applying (no DB required)
//! - `sync` - Idempotent apply (create-if-not-exists, requires DB for streams)
//! - `recreate` - Drop and recreate (requires DB for streams)

use clap::{Args, Subcommand};
use std::path::Path;

/// Gold layer DDL operations.
#[derive(Args)]
pub struct GoldArgs {
    #[command(subcommand)]
    pub command: GoldCommands,
}

#[derive(Subcommand)]
pub enum GoldCommands {
    /// Generate Gold DDL without applying.
    Generate {
        /// Target stream ID.
        #[arg(long, conflicts_with = "domain")]
        stream: Option<String>,

        /// Target domain ID.
        #[arg(long, conflicts_with = "stream")]
        domain: Option<String>,

        /// Include state transition view DDL (requires --stream).
        #[arg(long)]
        transitions: bool,

        /// Include events infrastructure DDL (requires --domain).
        #[arg(long)]
        events: bool,

        /// Validate config only, do not generate DDL.
        #[arg(long)]
        validate_only: bool,

        /// Skip pre-generation validation.
        #[arg(long)]
        no_validate: bool,
    },

    /// Sync Gold layer (idempotent create-if-not-exists).
    Sync {
        /// Target stream ID.
        #[arg(long, conflicts_with = "domain")]
        stream: Option<String>,

        /// Target domain ID.
        #[arg(long, conflicts_with = "stream")]
        domain: Option<String>,

        /// Include state transition view DDL (requires --stream).
        #[arg(long)]
        transitions: bool,

        /// Include events infrastructure DDL (requires --domain).
        #[arg(long)]
        events: bool,

        /// Generate DDL without applying to database.
        #[arg(long)]
        dry_run: bool,

        /// Skip pre-sync validation.
        #[arg(long)]
        no_validate: bool,
    },

    /// Recreate Gold layer (drop and create).
    Recreate {
        /// Target stream ID.
        #[arg(long, conflicts_with = "domain")]
        stream: Option<String>,

        /// Target domain ID.
        #[arg(long, conflicts_with = "stream")]
        domain: Option<String>,

        /// Generate DDL without applying to database.
        #[arg(long)]
        dry_run: bool,

        /// Skip pre-recreate validation.
        #[arg(long)]
        no_validate: bool,
    },
}

/// Run the gold subcommand.
///
/// `base_config_dir` points to the streams/dimensions config base (e.g. `config/base`).
/// Gold config loader needs the config root (parent of base), so we go up one level.
/// `db_url` is optional -- only required for sync/recreate with --stream.
/// `db_timeout` is the connection timeout in seconds.
pub async fn run(
    args: GoldArgs,
    base_config_dir: &Path,
    db_url: Option<&str>,
    db_timeout: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    // Gold configs live at config/base/streams/<id>/config.json and
    // config/domains/<domain>/domain.json. The Gold ConfigLoader needs
    // the config ROOT directory (parent of "base").
    let config_dir = base_config_dir.parent().unwrap_or(base_config_dir);

    let loader = ndp_lib::gold::config::FileSystemConfigLoader::new(config_dir);

    match args.command {
        GoldCommands::Generate {
            stream,
            domain,
            transitions,
            events,
            validate_only,
            no_validate: _,
        } => {
            if validate_only {
                run_validate_only(&loader, stream, domain).await
            } else {
                run_generate(&loader, stream, domain, transitions, events).await
            }
        }
        GoldCommands::Sync {
            stream,
            domain,
            transitions: _,
            events: _,
            dry_run,
            no_validate: _,
        } => run_sync(&loader, stream, domain, db_url, db_timeout, dry_run).await,
        GoldCommands::Recreate {
            stream,
            domain,
            dry_run: _,
            no_validate: _,
        } => run_recreate(&loader, stream, domain, db_url, db_timeout).await,
    }
}

/// Generate Gold DDL and print to stdout. No database connection needed.
async fn run_generate(
    loader: &ndp_lib::gold::config::FileSystemConfigLoader,
    stream: Option<String>,
    domain: Option<String>,
    transitions: bool,
    events: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Events requires --domain
    if events && domain.is_none() {
        return Err("--events requires --domain".into());
    }

    let opts = ndp_lib::gold::GenerateOptions {
        transitions,
        events,
        verbose: false,
    };

    if let Some(domain_id) = domain {
        tracing::info!(domain_id = %domain_id, "Generating Gold DDL for domain");

        let ddl = ndp_lib::gold::generate_domain(loader, &domain_id, &opts)?;
        println!("{ddl}");
        return Ok(());
    }

    if let Some(stream_id) = stream {
        tracing::info!(stream_id = %stream_id, transitions = transitions, "Generating Gold DDL for stream");

        let ddl = ndp_lib::gold::generate_stream(loader, &stream_id, &opts)?;
        println!("{ddl}");
        return Ok(());
    }

    Err("Must specify --stream or --domain".into())
}

/// Validate Gold config without generating DDL.
async fn run_validate_only(
    loader: &ndp_lib::gold::config::FileSystemConfigLoader,
    stream: Option<String>,
    domain: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    use ndp_lib::gold::config::ConfigLoader;

    if let Some(domain_id) = domain {
        let config = ConfigLoader::load_domain_config(loader, &domain_id)?;
        println!("Domain '{}' configuration is valid", config.id);
        return Ok(());
    }

    if let Some(stream_id) = stream {
        let config = ConfigLoader::load_stream_config(loader, &stream_id)?;
        if let Some(ref gold_etl) = config.gold_etl {
            if !gold_etl.enabled {
                return Err(format!("Stream '{}' Gold ETL is disabled", stream_id).into());
            }
            // Run config validation
            ndp_lib::gold::validation::ConfigValidator::new().validate(&config)?;
            println!("Stream '{}' Gold ETL configuration is valid", stream_id);
        } else {
            return Err(format!("Stream '{}' has no gold_etl configuration", stream_id).into());
        }
        return Ok(());
    }

    Err("Must specify --stream or --domain".into())
}

/// Sync Gold DDL (idempotent create-if-not-exists).
///
/// Stream sync requires a database connection for CA existence checks.
/// Domain sync does not use DB checks (aligned views use DO $$ IF NOT EXISTS).
async fn run_sync(
    loader: &ndp_lib::gold::config::FileSystemConfigLoader,
    stream: Option<String>,
    domain: Option<String>,
    db_url: Option<&str>,
    db_timeout: u64,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(stream_id) = stream {
        let db_url = require_db_url(db_url)?;
        tracing::info!(
            stream_id = %stream_id,
            db_url = %db_url,
            dry_run = dry_run,
            "Syncing Gold DDL for stream"
        );

        let opts = ndp_lib::types::SyncOptions { dry_run };

        if dry_run {
            // Dry-run: generate full DDL without DB checks
            let gen_opts = ndp_lib::gold::GenerateOptions {
                transitions: false,
                events: false,
                verbose: false,
            };
            let ddl = ndp_lib::gold::generate_stream(loader, &stream_id, &gen_opts)?;
            println!("{ddl}");
        } else {
            // Real sync: connect to DB, run SyncPlanner
            let db = ndp_lib::db::PostgresClient::connect(db_url, db_timeout).await?;
            let checker = ndp_lib::gold::PostgresCaChecker::new(db);
            let ddl = ndp_lib::gold::sync_stream(loader, &stream_id, &checker, &opts).await?;
            println!("{ddl}");
        }

        return Ok(());
    }

    if let Some(domain_id) = domain {
        tracing::info!(domain_id = %domain_id, "Syncing Gold DDL for domain");

        let opts = ndp_lib::types::SyncOptions { dry_run };
        let ddl = ndp_lib::gold::sync_domain(loader, &domain_id, &opts)?;
        println!("{ddl}");
        return Ok(());
    }

    Err("Must specify --stream or --domain".into())
}

/// Recreate Gold DDL (drop and create).
async fn run_recreate(
    loader: &ndp_lib::gold::config::FileSystemConfigLoader,
    stream: Option<String>,
    domain: Option<String>,
    db_url: Option<&str>,
    _db_timeout: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let opts = ndp_lib::gold::GenerateOptions {
        transitions: false,
        events: false,
        verbose: false,
    };

    if let Some(stream_id) = stream {
        // Recreate requires DB URL to be available (deploy.sh always passes it)
        let _db_url = require_db_url(db_url)?;
        tracing::info!(stream_id = %stream_id, "Recreating Gold DDL for stream");

        let ddl = ndp_lib::gold::recreate_stream(loader, &stream_id, &opts)?;
        println!("{ddl}");
        return Ok(());
    }

    if let Some(domain_id) = domain {
        tracing::info!(domain_id = %domain_id, "Recreating Gold DDL for domain");

        // Domain recreate generates aligned view DDL with Action::Recreate
        let domain_config =
            ndp_lib::gold::config::ConfigLoader::load_domain_config(loader, &domain_id)?;
        let generator = ndp_lib::gold::AlignedViewGenerator::new(loader.clone());
        let ddl = generator.generate(&domain_config, ndp_lib::gold::config::Action::Recreate)?;
        println!("{ddl}");
        return Ok(());
    }

    Err("Must specify --stream or --domain".into())
}

/// Require a database URL, returning a user-friendly error if missing.
fn require_db_url(db_url: Option<&str>) -> Result<&str, Box<dyn std::error::Error>> {
    db_url.ok_or_else(|| {
        "Database URL is required for this operation. Pass --db-url or set TIMESCALE_URL.".into()
    })
}
