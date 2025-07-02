use anyhow::{Context, Result};
use autonomous_platform::{load_default_config, integration::platform_orchestrator::PlatformOrchestrator};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::signal;
use tracing::{info, warn, error, Level};
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

    // Create platform orchestrator
    let orchestrator = PlatformOrchestrator::new(config).await
        .context("Failed to create platform orchestrator")?;

    info!("🏗️  Platform orchestrator initialized");

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

    // Start the platform
    match orchestrator.start_platform().await {
        Ok(()) => {
            info!("✅ Neural Trading Platform started successfully");
            
            // Perform initial health check
            match orchestrator.health_check().await {
                Ok(health) => {
                    info!("🏥 Initial health check completed:");
                    info!("   Overall healthy: {}", health.overall_healthy);
                    info!("   Components started: {}", health.components_started);
                    info!("   Data pipeline: {}", health.data_pipeline_healthy);
                    info!("   Streaming pipeline: {}", health.streaming_pipeline_healthy);
                    info!("   Neural system: {}", health.neural_system_healthy);
                }
                Err(e) => {
                    warn!("Initial health check failed: {}", e);
                }
            }

            // Main application loop
            info!("🔄 Entering main application loop...");
            
            loop {
                // Check for shutdown signal
                if shutdown_signal.load(Ordering::Relaxed) || orchestrator.is_shutting_down() {
                    info!("🛑 Shutdown signal detected, initiating graceful shutdown...");
                    break;
                }

                // Perform periodic health checks
                tokio::select! {
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                        match orchestrator.health_check().await {
                            Ok(health) => {
                                if !health.overall_healthy {
                                    warn!("⚠️  System health degraded: {}", 
                                          if !health.data_pipeline_healthy { "data_pipeline " } else { "" });
                                    warn!("   Attempting component recovery...");
                                }
                            }
                            Err(e) => {
                                error!("Health check failed: {}", e);
                            }
                        }
                    }
                    _ = signal::ctrl_c() => {
                        info!("🛑 Additional shutdown signal received");
                        break;
                    }
                }
            }
        }
        Err(e) => {
            error!("❌ Failed to start platform: {}", e);
            return Err(e);
        }
    }

    // Graceful shutdown
    info!("🔄 Shutting down Neural Trading Platform...");
    match orchestrator.shutdown_platform().await {
        Ok(()) => {
            info!("✅ Platform shutdown completed successfully");
        }
        Err(e) => {
            error!("❌ Error during platform shutdown: {}", e);
            return Err(e);
        }
    }

    info!("👋 Neural Trading Platform terminated");
    Ok(())
}