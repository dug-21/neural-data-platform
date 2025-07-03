use anyhow::{Context, Result};
use autonomous_platform::load_default_config;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::signal;
use tracing::{info, error, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    info!("🚀 Starting Neural Trading Platform...");

    // Load configuration
    let config = load_default_config()
        .context("Failed to load platform configuration")?;

    info!("📋 Configuration loaded successfully");
    info!("   Database: {}", config.database.url);
    info!("   Redis: {}", config.redis.url);
    info!("   Neural Memory: {}GB", config.neural.memory_gb);
    info!("   Models: {:?}", config.neural.models);

    // Setup graceful shutdown handler
    let shutdown_signal = Arc::new(AtomicBool::new(false));
    let shutdown_clone = Arc::clone(&shutdown_signal);

    // Spawn shutdown signal handler
    tokio::spawn(async move {
        match signal::ctrl_c().await {
            Ok(()) => {
                info!("🛑 Received shutdown signal (Ctrl+C)");
                shutdown_clone.store(true, Ordering::Relaxed);
            }
            Err(err) => {
                error!("Failed to install CTRL+C signal handler: {}", err);
            }
        }
    });

    info!("✅ Neural Trading Platform started successfully");
    info!("🔄 Platform is running. Press Ctrl+C to shut down.");
    
    // Main application loop - wait for shutdown signal
    loop {
        // Check for shutdown signal
        if shutdown_signal.load(Ordering::Relaxed) {
            info!("🛑 Shutdown signal detected, initiating graceful shutdown...");
            break;
        }

        // Sleep briefly to prevent busy waiting
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    info!("👋 Neural Trading Platform terminated");
    Ok(())
}