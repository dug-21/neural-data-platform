//! NDP Intelligence Application
//!
//! Binary for intelligence operations: embedding generation, schema management,
//! and intelligence cycle execution (Phase 2).

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ndp-intelligence")]
#[command(about = "NDP Intelligence Foundation - embedding generation and management")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the intelligence daemon (Phase 2 -- not yet implemented)
    Daemon,
    /// Run a one-shot embedding generation pass
    OneShot {
        /// Domain ID to generate embeddings for
        #[arg(long)]
        domain: String,
    },
    /// Backfill embeddings from historical data
    Backfill {
        /// Domain ID to backfill
        #[arg(long)]
        domain: String,
        /// Start timestamp (ISO 8601)
        #[arg(long)]
        since: Option<String>,
    },
    /// Show intelligence system status
    Status,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Daemon => {
            println!("Intelligence daemon is not yet implemented (Phase 2).");
            println!("This will run the continuous intelligence cycle.");
        }
        Commands::OneShot { domain } => {
            println!(
                "One-shot embedding generation for domain '{}' is not yet implemented (Phase 2).",
                domain
            );
            println!("This will generate embeddings for the latest Gold data.");
        }
        Commands::Backfill { domain, since } => {
            let since_msg = since
                .as_deref()
                .unwrap_or("beginning of available data");
            println!(
                "Backfill for domain '{}' since {} is not yet implemented (Phase 2).",
                domain, since_msg
            );
            println!("This will generate embeddings for historical Gold data.");
        }
        Commands::Status => {
            println!("Intelligence system status is not yet implemented (Phase 2).");
            println!("This will show embedding counts, graph stats, and prediction accuracy.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_cli_parses_daemon() {
        let cli = Cli::try_parse_from(["ndp-intelligence", "daemon"]);
        assert!(cli.is_ok(), "Should parse daemon subcommand");
        assert!(matches!(cli.unwrap().command, Commands::Daemon));
    }

    #[test]
    fn test_cli_parses_one_shot() {
        let cli = Cli::try_parse_from(["ndp-intelligence", "one-shot", "--domain", "test"]);
        assert!(cli.is_ok(), "Should parse one-shot subcommand");
        match cli.unwrap().command {
            Commands::OneShot { domain } => assert_eq!(domain, "test"),
            _ => panic!("Expected OneShot"),
        }
    }

    #[test]
    fn test_cli_parses_backfill() {
        let cli = Cli::try_parse_from(["ndp-intelligence", "backfill", "--domain", "test"]);
        assert!(cli.is_ok(), "Should parse backfill subcommand");
        match cli.unwrap().command {
            Commands::Backfill { domain, since } => {
                assert_eq!(domain, "test");
                assert!(since.is_none());
            }
            _ => panic!("Expected Backfill"),
        }
    }

    #[test]
    fn test_cli_parses_backfill_with_since() {
        let cli = Cli::try_parse_from([
            "ndp-intelligence",
            "backfill",
            "--domain",
            "test",
            "--since",
            "2026-01-01T00:00:00Z",
        ]);
        assert!(cli.is_ok(), "Should parse backfill with --since");
        match cli.unwrap().command {
            Commands::Backfill { domain, since } => {
                assert_eq!(domain, "test");
                assert_eq!(since.unwrap(), "2026-01-01T00:00:00Z");
            }
            _ => panic!("Expected Backfill"),
        }
    }

    #[test]
    fn test_cli_parses_status() {
        let cli = Cli::try_parse_from(["ndp-intelligence", "status"]);
        assert!(cli.is_ok(), "Should parse status subcommand");
        assert!(matches!(cli.unwrap().command, Commands::Status));
    }

    #[test]
    fn test_cli_requires_subcommand() {
        let cli = Cli::try_parse_from(["ndp-intelligence"]);
        assert!(cli.is_err(), "Should require a subcommand");
    }
}
