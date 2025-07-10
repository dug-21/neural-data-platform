use anyhow::{Context, Result};
use autonomous_platform::{load_default_config, PlatformOrchestrator};
use autonomous_platform::orchestration::platform_orchestrator::OrchestrationConfig;
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

    // Initialize the platform orchestrator
    let orchestrator_config = OrchestrationConfig::default();
    let orchestrator = Arc::new(
        PlatformOrchestrator::new(orchestrator_config)
    );

    // Initialize the orchestrator
    if let Err(e) = orchestrator.initialize().await {
        error!("Failed to initialize orchestrator: {}", e);
        return Err(e.into());
    }

    info!("🎯 Platform orchestrator initialized successfully");

    // Setup graceful shutdown handler
    let shutdown_signal = Arc::new(AtomicBool::new(false));
    let shutdown_clone = Arc::clone(&shutdown_signal);
    let orchestrator_clone = orchestrator.clone();

    // Spawn shutdown signal handler
    tokio::spawn(async move {
        match signal::ctrl_c().await {
            Ok(()) => {
                info!("🛑 Received shutdown signal (Ctrl+C)");
                shutdown_clone.store(true, Ordering::Relaxed);
                
                // Initiate orchestrator shutdown
                if let Err(e) = orchestrator_clone.shutdown().await {
                    error!("Error during orchestrator shutdown: {}", e);
                }
            }
            Err(err) => {
                error!("Failed to install CTRL+C signal handler: {}", err);
            }
        }
    });

    // Start the orchestration loop
    let orchestrator_loop = orchestrator.clone();
    let loop_signal = shutdown_signal.clone();
    tokio::spawn(async move {
        info!("🔄 Starting orchestration event loop...");
        if let Err(e) = orchestrator_loop.run_orchestration_loop().await {
            error!("Orchestration loop error: {}", e);
        }
    });

    info!("✅ Neural Trading Platform started successfully");
    info!("🔄 Platform is running with active orchestration. Press Ctrl+C to shut down.");
    
    // Main application loop - wait for shutdown signal
    loop {
        // Check for shutdown signal
        if shutdown_signal.load(Ordering::Relaxed) {
            info!("🛑 Shutdown signal detected, waiting for graceful shutdown...");
            break;
        }

        // Sleep briefly to prevent busy waiting
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    info!("👋 Neural Trading Platform terminated");
    Ok(())
}