//! Silver ETL CLI
//!
//! Config-driven ETL binary for Bronze to Silver layer transformation.
//!
//! ## Usage
//!
//! ```bash
//! # Migrate schema from config (creates tables, hypertables, indexes)
//! silver-etl migrate --stream air-quality
//! silver-etl migrate  # all enabled streams
//!
//! # Dry-run migration: show DDL without executing
//! silver-etl migrate --stream air-quality --dry-run
//!
//! # Run ETL for all enabled streams
//! silver-etl run
//!
//! # Run ETL for specific stream
//! silver-etl run --stream air-quality
//!
//! # Full reload (ignore watermark)
//! silver-etl run --stream air-quality --full-reload
//!
//! # Dry-run: generate SQL without executing
//! silver-etl dry-run --stream air-quality
//!
//! # Validate configuration
//! silver-etl validate
//! silver-etl validate --stream air-quality
//!
//! # Show ETL status and watermarks
//! silver-etl status
//! silver-etl status --stream air-quality
//! ```

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use prometheus::{Encoder, Registry, TextEncoder};
use silver_etl::{
    ConfigLoader, DaemonConfig, DaemonRunner, EtlMetrics, EtlRunner, EtlStats, RealEtlExecutor,
    SchemaGenerator,
};
use std::io::Write;
use std::time::Instant;
use tokio::sync::watch;
use tracing::{debug, error, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

/// Silver ETL - Config-driven Bronze to Silver transformation
#[derive(Parser, Debug)]
#[command(name = "silver-etl")]
#[command(author = "Neural Data Platform Team")]
#[command(version = "0.1.0")]
#[command(about = "Config-driven ETL for Silver layer transformation", long_about = None)]
struct Cli {
    /// Verbosity level (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    /// etcd endpoint for configuration
    #[arg(
        long,
        env = "ETCD_ENDPOINT",
        default_value = "http://localhost:2379",
        global = true
    )]
    etcd_endpoint: String,

    /// Bronze data directory
    #[arg(
        long,
        env = "BRONZE_DATA_DIR",
        default_value = "/data/raw",
        global = true
    )]
    bronze_dir: String,

    /// TimescaleDB connection string
    #[arg(long, env = "TIMESCALE_URL", global = true)]
    timescale_url: Option<String>,

    /// YAML config directory fallback (when etcd unavailable)
    #[arg(
        long,
        env = "CONFIG_DIR",
        default_value = "/workspaces/neural-data-platform/config/base/streams",
        global = true
    )]
    config_dir: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run ETL for one or all enabled streams
    Run {
        /// Specific stream to process (omit for all enabled streams)
        #[arg(short, long)]
        stream: Option<String>,

        /// Force full reload (ignore watermark)
        #[arg(long)]
        full_reload: bool,
    },

    /// Generate SQL without executing (dry-run)
    DryRun {
        /// Stream to generate SQL for (required)
        #[arg(short, long)]
        stream: String,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Validate stream configuration
    Validate {
        /// Stream to validate (omit for all streams)
        #[arg(short, long)]
        stream: Option<String>,
    },

    /// Show ETL status and watermarks
    Status {
        /// Stream to show status for (omit for all streams)
        #[arg(short, long)]
        stream: Option<String>,

        /// Output format (text, json)
        #[arg(long, default_value = "text")]
        format: String,
    },

    /// Export Prometheus metrics
    Metrics,

    /// Create/update TimescaleDB schema from config (config-driven DDL)
    Migrate {
        /// Stream to migrate (omit for all enabled streams)
        #[arg(short, long)]
        stream: Option<String>,

        /// Dry-run: show DDL without executing
        #[arg(long)]
        dry_run: bool,

        /// Output file for DDL (default: stdout for dry-run)
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Run ETL in daemon mode (continuous processing with interval)
    Daemon {
        /// Interval between ETL runs in seconds
        #[arg(short, long, default_value = "300")]
        interval: u64,

        /// Specific stream to process (omit for all enabled streams)
        #[arg(short, long)]
        stream: Option<String>,
    },
}

/// Main entry point
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing based on verbosity
    let level = match cli.verbose {
        0 => Level::INFO,
        1 => Level::DEBUG,
        _ => Level::TRACE,
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(cli.verbose >= 2)
        .with_line_number(cli.verbose >= 2)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    info!(
        etcd_endpoint = %cli.etcd_endpoint,
        bronze_dir = %cli.bronze_dir,
        config_dir = %cli.config_dir,
        "Silver ETL starting"
    );

    // Initialize Prometheus metrics registry
    let registry = Registry::new();
    let _metrics = EtlMetrics::init(&registry);

    // Execute command
    let result = match &cli.command {
        Commands::Run {
            ref stream,
            full_reload,
        } => run_etl(&cli, stream.clone(), *full_reload).await,
        Commands::DryRun {
            ref stream,
            ref output,
        } => dry_run(&cli, stream, output.as_deref()).await,
        Commands::Validate { ref stream } => validate_config(&cli, stream.as_deref()).await,
        Commands::Status {
            ref stream,
            ref format,
        } => show_status(&cli, stream.as_deref(), format).await,
        Commands::Metrics => export_metrics(&registry),
        Commands::Migrate {
            ref stream,
            dry_run,
            ref output,
        } => migrate_schema(&cli, stream.as_deref(), *dry_run, output.as_deref()).await,
        Commands::Daemon {
            interval,
            ref stream,
        } => run_daemon(&cli, *interval, stream.clone()).await,
    };

    // Handle result and set exit code
    match result {
        Ok(exit_code) => {
            std::process::exit(exit_code);
        }
        Err(e) => {
            error!(error = %e, "Command failed");
            std::process::exit(1);
        }
    }
}

/// Run ETL for one or all streams
async fn run_etl(cli: &Cli, stream: Option<String>, full_reload: bool) -> Result<i32> {
    info!(
        stream = stream.as_deref().unwrap_or("all"),
        full_reload = full_reload,
        "Running ETL"
    );

    // Load configuration
    let config_loader = ConfigLoader::new(&cli.etcd_endpoint, &cli.config_dir);

    // Get list of streams to process
    let streams = match stream {
        Some(s) => vec![s],
        None => config_loader
            .load_all_enabled()
            .await
            .context("Failed to load enabled streams")?,
    };

    if streams.is_empty() {
        warn!("No streams with silver_etl.enabled = true found");
        return Ok(0);
    }

    info!(count = streams.len(), "Found streams to process");

    // Create ETL runner
    let runner = match &cli.timescale_url {
        Some(url) => EtlRunner::with_postgres(url).context("Failed to create PostgreSQL runner")?,
        None => {
            warn!("No TIMESCALE_URL provided, using dry-run mode (no database writes)");
            EtlRunner::new_in_memory().context("Failed to create in-memory runner")?
        }
    };

    // Track overall results
    let mut all_stats: Vec<EtlStats> = Vec::new();
    let mut failed_streams: Vec<String> = Vec::new();
    let overall_start = Instant::now();

    // Process each stream
    for stream_id in &streams {
        info!(stream_id = %stream_id, "Processing stream");
        let stream_start = Instant::now();

        // Load stream config
        let stream_config = match config_loader.load_stream_config(stream_id).await {
            Ok(config) => config,
            Err(e) => {
                error!(stream_id = %stream_id, error = %e, "Failed to load stream config");
                failed_streams.push(stream_id.clone());
                continue;
            }
        };

        // Run ETL for this stream (note: full_reload affects watermark handling in config, not in run_etl)
        // TODO: Implement full_reload by passing watermark=None to run_etl
        let _ = full_reload; // Currently unused, placeholder for future implementation
        match runner.run_etl(&stream_config, stream_id, &cli.bronze_dir) {
            Ok(stats) => {
                let duration = stream_start.elapsed();
                info!(
                    stream_id = %stream_id,
                    rows_processed = stats.rows_processed,
                    rows_with_dq_flags = stats.rows_with_dq_flags,
                    rows_rejected = stats.rows_rejected,
                    duration_ms = duration.as_millis() as u64,
                    "Stream ETL completed"
                );

                // Update Prometheus metrics
                if let Some(metrics) = EtlMetrics::get() {
                    metrics
                        .rows_processed
                        .with_label_values(&[stream_id])
                        .inc_by(stats.rows_processed);
                    metrics
                        .rows_flagged
                        .with_label_values(&[stream_id])
                        .inc_by(stats.rows_with_dq_flags);
                    metrics
                        .rows_rejected
                        .with_label_values(&[stream_id])
                        .inc_by(stats.rows_rejected);
                    metrics.duration_seconds.observe(duration.as_secs_f64());
                    metrics.runs_total.inc();
                }

                all_stats.push(stats);
            }
            Err(e) => {
                error!(stream_id = %stream_id, error = %e, "Stream ETL failed");
                failed_streams.push(stream_id.clone());
            }
        }
    }

    // Print summary
    let total_duration = overall_start.elapsed();
    let total_rows: u64 = all_stats.iter().map(|s| s.rows_processed).sum();
    let total_flagged: u64 = all_stats.iter().map(|s| s.rows_with_dq_flags).sum();
    let total_rejected: u64 = all_stats.iter().map(|s| s.rows_rejected).sum();

    println!();
    println!("=== ETL Summary ===");
    println!("Streams processed: {}/{}", all_stats.len(), streams.len());
    println!("Total rows processed: {}", total_rows);
    println!("Total rows with DQ flags: {}", total_flagged);
    println!("Total rows rejected: {}", total_rejected);
    println!("Total duration: {:.2}s", total_duration.as_secs_f64());

    if !failed_streams.is_empty() {
        println!();
        println!("Failed streams:");
        for stream_id in &failed_streams {
            println!("  - {}", stream_id);
        }
    }

    // Return exit code: 0 if all succeeded, 1 if any failed
    Ok(if failed_streams.is_empty() { 0 } else { 1 })
}

/// Generate SQL without executing (dry-run mode)
async fn dry_run(cli: &Cli, stream_id: &str, output: Option<&str>) -> Result<i32> {
    info!(stream_id = %stream_id, output = output.unwrap_or("stdout"), "Generating SQL (dry-run)");

    // Load configuration
    let config_loader = ConfigLoader::new(&cli.etcd_endpoint, &cli.config_dir);

    // Load stream config
    let stream_config = config_loader
        .load_stream_config(stream_id)
        .await
        .context(format!("Failed to load config for stream '{}'", stream_id))?;

    // Create runner and generate SQL
    let runner = EtlRunner::new_in_memory().context("Failed to create runner")?;

    let sql = runner
        .dry_run(&stream_config, stream_id, &cli.bronze_dir)
        .context("Failed to generate SQL")?;

    // Output SQL
    match output {
        Some(path) => {
            let mut file = std::fs::File::create(path)
                .context(format!("Failed to create output file '{}'", path))?;
            file.write_all(sql.as_bytes())
                .context("Failed to write SQL to file")?;
            info!(path = %path, "SQL written to file");
        }
        None => {
            println!("{}", sql);
        }
    }

    Ok(0)
}

/// Validate stream configuration
async fn validate_config(cli: &Cli, stream: Option<&str>) -> Result<i32> {
    info!(stream = stream.unwrap_or("all"), "Validating configuration");

    let config_loader = ConfigLoader::new(&cli.etcd_endpoint, &cli.config_dir);

    // Get list of streams to validate
    let streams = match stream {
        Some(s) => vec![s.to_string()],
        None => config_loader
            .list_all_streams()
            .await
            .context("Failed to list streams")?,
    };

    if streams.is_empty() {
        warn!("No streams found to validate");
        return Ok(0);
    }

    let mut valid_count = 0;
    let mut invalid_count = 0;

    for stream_id in &streams {
        debug!(stream_id = %stream_id, "Validating stream");

        match config_loader.load_stream_config(stream_id).await {
            Ok(config) => match config.validate() {
                Ok(()) => {
                    println!("[OK] {}: Configuration valid", stream_id);
                    valid_count += 1;
                }
                Err(e) => {
                    println!("[ERROR] {}: {}", stream_id, e);
                    invalid_count += 1;
                }
            },
            Err(e) => {
                println!("[ERROR] {}: Failed to load config - {}", stream_id, e);
                invalid_count += 1;
            }
        }
    }

    println!();
    println!(
        "Validation summary: {} valid, {} invalid",
        valid_count, invalid_count
    );

    Ok(if invalid_count == 0 { 0 } else { 1 })
}

/// Show ETL status and watermarks
async fn show_status(cli: &Cli, stream: Option<&str>, format: &str) -> Result<i32> {
    info!(stream = stream.unwrap_or("all"), format = %format, "Showing ETL status");

    let config_loader = ConfigLoader::new(&cli.etcd_endpoint, &cli.config_dir);

    // Get list of streams
    let streams = match stream {
        Some(s) => vec![s.to_string()],
        None => config_loader
            .load_all_enabled()
            .await
            .context("Failed to load enabled streams")?,
    };

    if streams.is_empty() {
        println!("No enabled streams found");
        return Ok(0);
    }

    // Create runner to query watermarks (if TimescaleDB available)
    let runner = match &cli.timescale_url {
        Some(url) => Some(EtlRunner::with_postgres(url)?),
        None => None,
    };

    // Collect status for each stream
    let mut status_entries: Vec<StreamStatus> = Vec::new();

    for stream_id in &streams {
        let config = match config_loader.load_stream_config(stream_id).await {
            Ok(c) => c,
            Err(e) => {
                warn!(stream_id = %stream_id, error = %e, "Failed to load config");
                continue;
            }
        };

        let watermark = match &runner {
            Some(r) => r
                .get_watermark(&config.target_table, &config.timestamp.target_field)
                .ok(),
            None => None,
        };

        // Flatten Option<Result<Option<DateTime>>> to Option<String>
        let watermark_str = watermark.and_then(|opt| opt).map(|w| w.to_rfc3339());

        status_entries.push(StreamStatus {
            stream_id: stream_id.clone(),
            target_table: config.target_table.clone(),
            enabled: config.enabled,
            incremental_enabled: config.incremental.enabled,
            watermark_column: config.incremental.watermark_column.clone(),
            current_watermark: watermark_str,
        });
    }

    // Output based on format
    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&status_entries)
                .context("Failed to serialize status to JSON")?;
            println!("{}", json);
        }
        _ => {
            // Text format
            println!(
                "{:<20} {:<35} {:<8} {:<12} {:<25}",
                "STREAM", "TARGET_TABLE", "ENABLED", "INCREMENTAL", "WATERMARK"
            );
            println!("{}", "-".repeat(100));

            for status in &status_entries {
                println!(
                    "{:<20} {:<35} {:<8} {:<12} {:<25}",
                    status.stream_id,
                    status.target_table,
                    if status.enabled { "yes" } else { "no" },
                    if status.incremental_enabled {
                        "yes"
                    } else {
                        "no"
                    },
                    status.current_watermark.as_deref().unwrap_or("N/A"),
                );
            }
        }
    }

    Ok(0)
}

/// Migrate schema - create/update TimescaleDB tables from config
///
/// This is the truly config-driven approach: the schema is derived entirely
/// from the silver_etl configuration. No manual SQL migrations needed.
///
/// Supports schema evolution: if a table exists, new columns from config
/// are added via ALTER TABLE ADD COLUMN IF NOT EXISTS.
async fn migrate_schema(
    cli: &Cli,
    stream: Option<&str>,
    dry_run: bool,
    output: Option<&str>,
) -> Result<i32> {
    info!(
        stream = stream.unwrap_or("all"),
        dry_run = dry_run,
        "Migrating schema from config"
    );

    let config_loader = ConfigLoader::new(&cli.etcd_endpoint, &cli.config_dir);
    let schema_gen = SchemaGenerator::new();

    // Get list of streams
    let streams = match stream {
        Some(s) => vec![s.to_string()],
        None => config_loader
            .load_all_enabled()
            .await
            .context("Failed to load enabled streams")?,
    };

    if streams.is_empty() {
        println!("No enabled streams found");
        return Ok(0);
    }

    // Connect to TimescaleDB first (needed for querying existing columns)
    let timescale_url = cli
        .timescale_url
        .as_ref()
        .context("--timescale-url required for migration (or set TIMESCALE_URL)")?;

    info!(url = %timescale_url, "Connecting to TimescaleDB");

    let (client, connection) = tokio_postgres::connect(timescale_url, tokio_postgres::NoTls)
        .await
        .context("Failed to connect to TimescaleDB")?;

    // Spawn connection handler
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            error!(error = %e, "PostgreSQL connection error");
        }
    });

    // Collect all DDL statements
    let mut all_ddl = Vec::new();
    let mut schemas_created = std::collections::HashSet::new();

    for stream_id in &streams {
        info!(stream_id = %stream_id, "Processing stream");

        let config = config_loader
            .load_stream_config(stream_id)
            .await
            .context(format!("Failed to load config for stream '{}'", stream_id))?;

        // Validate config first
        config
            .validate()
            .context(format!("Invalid configuration for stream '{}'", stream_id))?;

        // Query existing columns for schema evolution
        let existing_columns = query_existing_columns(&client, &config.target_table).await?;

        // Add schema creation (once per schema)
        let schema_ddl = schema_gen.generate_create_schema(&config)?;
        let schema_name = schema_ddl
            .split_whitespace()
            .last()
            .unwrap_or("silver")
            .trim_end_matches(';')
            .to_string();
        if !schemas_created.contains(&schema_name) {
            all_ddl.push(format!("-- Schema for {}", stream_id));
            all_ddl.push(schema_ddl);
            all_ddl.push(String::new());
            schemas_created.insert(schema_name);
        }

        // Add table creation
        all_ddl.push(format!("-- Table for stream: {}", stream_id));
        all_ddl.push(schema_gen.generate_create_table(&config)?);
        all_ddl.push(String::new());

        // Add ALTER TABLE for new columns (schema evolution)
        let alter_statements = schema_gen.generate_add_columns(&config, &existing_columns)?;
        if !alter_statements.is_empty() {
            all_ddl.push(format!(
                "-- Schema evolution: adding {} new column(s) to {}",
                alter_statements.len(),
                config.target_table
            ));
            for stmt in &alter_statements {
                all_ddl.push(stmt.clone());
            }
            all_ddl.push(String::new());
            info!(
                stream_id = %stream_id,
                new_columns = alter_statements.len(),
                "Generated ALTER TABLE for new columns"
            );
        }

        // Add hypertable creation
        all_ddl.push(format!("-- Hypertable for: {}", config.target_table));
        all_ddl.push(schema_gen.generate_hypertable(&config)?);
        all_ddl.push(String::new());

        // Add indexes
        all_ddl.push(format!("-- Indexes for: {}", config.target_table));
        for index_ddl in schema_gen.generate_indexes(&config)? {
            all_ddl.push(index_ddl);
        }
        all_ddl.push(String::new());

        info!(
            stream_id = %stream_id,
            table = %config.target_table,
            existing_columns = existing_columns.len(),
            "Generated DDL for stream"
        );
    }

    let combined_ddl = all_ddl.join("\n");

    if dry_run {
        // Output DDL without executing
        match output {
            Some(path) => {
                std::fs::write(path, &combined_ddl)
                    .context(format!("Failed to write DDL to {}", path))?;
                println!("DDL written to: {}", path);
            }
            None => {
                println!("{}", combined_ddl);
            }
        }
        return Ok(0);
    }

    // Execute DDL against TimescaleDB (already connected above)
    // Execute each statement
    let statements: Vec<&str> = combined_ddl
        .split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && !s.starts_with("--"))
        .collect();

    let mut success_count = 0;
    let mut error_count = 0;

    for stmt in statements {
        // Skip comments and empty lines
        if stmt.starts_with("--") || stmt.is_empty() {
            continue;
        }

        debug!(statement = %stmt, "Executing DDL");

        match client.execute(&format!("{};", stmt), &[]).await {
            Ok(_) => {
                success_count += 1;
                debug!("Statement executed successfully");
            }
            Err(e) => {
                // Some errors are expected (e.g., table already exists without IF NOT EXISTS)
                warn!(error = %e, statement = %stmt, "Statement failed");
                error_count += 1;
            }
        }
    }

    println!(
        "Migration complete: {} statements succeeded, {} failed",
        success_count, error_count
    );

    // Also write DDL to output file if specified
    if let Some(path) = output {
        std::fs::write(path, &combined_ddl).context(format!("Failed to write DDL to {}", path))?;
        println!("DDL also saved to: {}", path);
    }

    Ok(if error_count == 0 { 0 } else { 1 })
}

/// Query existing columns for a table from PostgreSQL
///
/// Returns empty vec if table doesn't exist (allows CREATE TABLE to proceed)
async fn query_existing_columns(
    client: &tokio_postgres::Client,
    table_name: &str,
) -> Result<Vec<String>> {
    // Parse schema.table format
    let parts: Vec<&str> = table_name.split('.').collect();
    let (schema, table) = match parts.as_slice() {
        [schema, table] => (*schema, *table),
        [table] => ("public", *table),
        _ => return Ok(vec![]), // Invalid format, let CREATE TABLE handle it
    };

    let sql = r#"
        SELECT column_name::text
        FROM information_schema.columns
        WHERE table_schema = $1 AND table_name = $2
        ORDER BY ordinal_position
    "#;

    match client.query(sql, &[&schema, &table]).await {
        Ok(rows) => {
            let columns: Vec<String> = rows.iter().map(|r| r.get(0)).collect();
            debug!(
                table = %table_name,
                columns = ?columns,
                "Found existing columns"
            );
            Ok(columns)
        }
        Err(e) => {
            // Table might not exist yet, that's fine
            debug!(table = %table_name, error = %e, "Could not query columns (table may not exist)");
            Ok(vec![])
        }
    }
}

/// Export Prometheus metrics
fn export_metrics(registry: &Registry) -> Result<i32> {
    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    let mut buffer = Vec::new();
    encoder
        .encode(&metric_families, &mut buffer)
        .context("Failed to encode metrics")?;

    let output = String::from_utf8(buffer).context("Metrics output is not valid UTF-8")?;

    println!("{}", output);
    Ok(0)
}

/// Run ETL in daemon mode with graceful shutdown
async fn run_daemon(cli: &Cli, interval: u64, stream: Option<String>) -> Result<i32> {
    info!(
        interval_secs = interval,
        stream = stream.as_deref().unwrap_or("all"),
        "Starting daemon mode"
    );

    // Create config loader
    let config_loader = ConfigLoader::new(&cli.etcd_endpoint, &cli.config_dir);

    // Create ETL runner
    let runner = match &cli.timescale_url {
        Some(url) => EtlRunner::with_postgres(url).context("Failed to create PostgreSQL runner")?,
        None => {
            warn!("No TIMESCALE_URL provided, using dry-run mode (no database writes)");
            EtlRunner::new_in_memory().context("Failed to create in-memory runner")?
        }
    };

    // Create the real executor that wraps EtlRunner
    let executor = RealEtlExecutor::new(runner, config_loader, cli.bronze_dir.clone());

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Create daemon configuration
    let daemon_config = DaemonConfig {
        interval_secs: interval,
        stream_filter: stream,
        ..Default::default()
    };

    // Create daemon runner
    let mut daemon = DaemonRunner::new(executor, daemon_config, shutdown_rx);

    // Spawn task to handle shutdown signals
    let shutdown_handle = tokio::spawn(async move {
        // Wait for Ctrl+C
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C");
        info!("Received shutdown signal (Ctrl+C)");
        let _ = shutdown_tx.send(true);
    });

    // Run the daemon
    info!("Daemon running. Press Ctrl+C to gracefully shutdown.");
    let result = daemon.run().await;

    // Clean up
    shutdown_handle.abort();

    match result {
        Ok(()) => {
            info!("Daemon stopped gracefully");
            Ok(0)
        }
        Err(e) => {
            error!(error = %e, "Daemon stopped with error");
            Ok(1)
        }
    }
}

/// Stream status information
#[derive(Debug, serde::Serialize)]
struct StreamStatus {
    stream_id: String,
    target_table: String,
    enabled: bool,
    incremental_enabled: bool,
    watermark_column: String,
    current_watermark: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_cli_parsing() {
        // Verify CLI structure is valid
        Cli::command().debug_assert();
    }

    #[test]
    fn test_run_command_parsing() {
        let cli = Cli::parse_from(["silver-etl", "run"]);
        match cli.command {
            Commands::Run {
                stream,
                full_reload,
            } => {
                assert!(stream.is_none());
                assert!(!full_reload);
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_run_with_stream_parsing() {
        let cli = Cli::parse_from(["silver-etl", "run", "--stream", "air-quality"]);
        match cli.command {
            Commands::Run { stream, .. } => {
                assert_eq!(stream, Some("air-quality".to_string()));
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_run_full_reload_parsing() {
        let cli = Cli::parse_from(["silver-etl", "run", "--full-reload"]);
        match cli.command {
            Commands::Run { full_reload, .. } => {
                assert!(full_reload);
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_dry_run_command_parsing() {
        let cli = Cli::parse_from(["silver-etl", "dry-run", "--stream", "air-quality"]);
        match cli.command {
            Commands::DryRun { stream, output } => {
                assert_eq!(stream, "air-quality");
                assert!(output.is_none());
            }
            _ => panic!("Expected DryRun command"),
        }
    }

    #[test]
    fn test_dry_run_with_output() {
        let cli = Cli::parse_from([
            "silver-etl",
            "dry-run",
            "--stream",
            "air-quality",
            "--output",
            "/tmp/etl.sql",
        ]);
        match cli.command {
            Commands::DryRun { stream, output } => {
                assert_eq!(stream, "air-quality");
                assert_eq!(output, Some("/tmp/etl.sql".to_string()));
            }
            _ => panic!("Expected DryRun command"),
        }
    }

    #[test]
    fn test_validate_command_parsing() {
        let cli = Cli::parse_from(["silver-etl", "validate"]);
        match cli.command {
            Commands::Validate { stream } => {
                assert!(stream.is_none());
            }
            _ => panic!("Expected Validate command"),
        }
    }

    #[test]
    fn test_validate_with_stream() {
        let cli = Cli::parse_from(["silver-etl", "validate", "--stream", "outdoor-weather"]);
        match cli.command {
            Commands::Validate { stream } => {
                assert_eq!(stream, Some("outdoor-weather".to_string()));
            }
            _ => panic!("Expected Validate command"),
        }
    }

    #[test]
    fn test_status_command_parsing() {
        let cli = Cli::parse_from(["silver-etl", "status"]);
        match cli.command {
            Commands::Status { stream, format } => {
                assert!(stream.is_none());
                assert_eq!(format, "text");
            }
            _ => panic!("Expected Status command"),
        }
    }

    #[test]
    fn test_status_json_format() {
        let cli = Cli::parse_from(["silver-etl", "status", "--format", "json"]);
        match cli.command {
            Commands::Status { format, .. } => {
                assert_eq!(format, "json");
            }
            _ => panic!("Expected Status command"),
        }
    }

    #[test]
    fn test_global_options() {
        let cli = Cli::parse_from([
            "silver-etl",
            "-vv",
            "--etcd-endpoint",
            "http://etcd:2379",
            "--bronze-dir",
            "/custom/data",
            "--timescale-url",
            "postgresql://user:pass@localhost/ndp",
            "run",
        ]);

        assert_eq!(cli.verbose, 2);
        assert_eq!(cli.etcd_endpoint, "http://etcd:2379");
        assert_eq!(cli.bronze_dir, "/custom/data");
        assert_eq!(
            cli.timescale_url,
            Some("postgresql://user:pass@localhost/ndp".to_string())
        );
    }

    #[test]
    fn test_metrics_command() {
        let cli = Cli::parse_from(["silver-etl", "metrics"]);
        assert!(matches!(cli.command, Commands::Metrics));
    }

    #[test]
    fn test_daemon_command_parsing() {
        let cli = Cli::parse_from(["silver-etl", "daemon"]);
        match cli.command {
            Commands::Daemon { interval, stream } => {
                assert_eq!(interval, 300); // Default interval
                assert!(stream.is_none());
            }
            _ => panic!("Expected Daemon command"),
        }
    }

    #[test]
    fn test_daemon_with_custom_interval() {
        let cli = Cli::parse_from(["silver-etl", "daemon", "--interval", "60"]);
        match cli.command {
            Commands::Daemon { interval, stream } => {
                assert_eq!(interval, 60);
                assert!(stream.is_none());
            }
            _ => panic!("Expected Daemon command"),
        }
    }

    #[test]
    fn test_daemon_with_stream() {
        let cli = Cli::parse_from(["silver-etl", "daemon", "--stream", "air-quality"]);
        match cli.command {
            Commands::Daemon { interval, stream } => {
                assert_eq!(interval, 300);
                assert_eq!(stream, Some("air-quality".to_string()));
            }
            _ => panic!("Expected Daemon command"),
        }
    }

    #[test]
    fn test_daemon_with_all_options() {
        let cli = Cli::parse_from([
            "silver-etl",
            "daemon",
            "--interval",
            "120",
            "--stream",
            "outdoor-weather",
        ]);
        match cli.command {
            Commands::Daemon { interval, stream } => {
                assert_eq!(interval, 120);
                assert_eq!(stream, Some("outdoor-weather".to_string()));
            }
            _ => panic!("Expected Daemon command"),
        }
    }
}
