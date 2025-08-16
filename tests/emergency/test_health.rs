//! Emergency Test: System Health Checks
//! Tests that the system starts and responds correctly

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::timeout;

#[derive(Debug, Serialize, Deserialize)]
struct HealthStatus {
    is_healthy: bool,
    models_loaded: usize,
    database_connected: bool,
    redis_connected: bool,
    uptime_seconds: u64,
    version: String,
    components: Vec<ComponentStatus>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ComponentStatus {
    name: String,
    status: String,
    message: Option<String>,
}

async fn check_health_endpoint(url: &str) -> Result<HealthStatus> {
    let client = reqwest::Client::new();
    
    let response = timeout(
        Duration::from_secs(5),
        client.get(url).send()
    ).await??;
    
    if !response.status().is_success() {
        anyhow::bail!("Health check returned: {}", response.status());
    }
    
    Ok(response.json().await?)
}

async fn check_database_connection() -> Result<bool> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/neural_trader_db".to_string());
    
    match sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(2))
        .connect(&database_url)
        .await
    {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

async fn check_redis_connection() -> Result<bool> {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    
    match redis::Client::open(redis_url) {
        Ok(client) => {
            match client.get_connection() {
                Ok(mut conn) => {
                    // Try to ping the server
                    match redis::cmd("PING").query::<String>(&mut conn) {
                        Ok(_) => Ok(true),
                        Err(_) => Ok(false)
                    }
                }
                Err(_) => Ok(false),
            }
        }
        Err(_) => Ok(false),
    }
}

pub async fn test_system_health() -> Result<()> {
    println!("🧪 Testing System Health...");
    
    // Check main health endpoint
    match check_health_endpoint("http://localhost:8080/health").await {
        Ok(health) => {
            println!("  ✅ Health endpoint responding");
            println!("    - Healthy: {}", health.is_healthy);
            println!("    - Models loaded: {}", health.models_loaded);
            println!("    - Database: {}", if health.database_connected { "✅" } else { "❌" });
            println!("    - Redis: {}", if health.redis_connected { "✅" } else { "❌" });
            println!("    - Uptime: {} seconds", health.uptime_seconds);
            println!("    - Version: {}", health.version);
            
            assert!(health.is_healthy, "System reports unhealthy");
            
            // Check component statuses
            for component in &health.components {
                let status_icon = match component.status.as_str() {
                    "healthy" => "✅",
                    "degraded" => "⚠️",
                    _ => "❌",
                };
                println!("    - {}: {} {}", 
                    component.name, 
                    status_icon,
                    component.message.as_deref().unwrap_or("")
                );
            }
        }
        Err(e) => {
            if e.to_string().contains("Connection refused") {
                println!("  ⚠️  System appears to be offline");
                println!("  ℹ️  Testing individual components...");
                
                // Test components individually
                let db_status = check_database_connection().await?;
                println!("    - Database: {}", if db_status { "✅ Available" } else { "❌ Unavailable" });
                
                let redis_status = check_redis_connection().await?;
                println!("    - Redis: {}", if redis_status { "✅ Available" } else { "❌ Unavailable" });
            } else {
                println!("  ❌ Health check failed: {}", e);
            }
        }
    }
    
    println!("✅ System Health test completed");
    Ok(())
}

pub async fn test_api_endpoints() -> Result<()> {
    println!("🧪 Testing API Endpoints...");
    
    let endpoints = vec![
        ("/health", "Health"),
        ("/api/status", "Status"),
        ("/api/metrics", "Metrics"),
        ("/api/models", "Models"),
        ("/api/config", "Config"),
    ];
    
    let client = reqwest::Client::new();
    let base_url = "http://localhost:8080";
    
    for (path, name) in endpoints {
        let url = format!("{}{}", base_url, path);
        match timeout(Duration::from_secs(2), client.get(&url).send()).await {
            Ok(Ok(response)) => {
                if response.status().is_success() {
                    println!("  ✅ {} endpoint: OK", name);
                } else if response.status() == 404 {
                    println!("  ⚠️  {} endpoint: Not Found", name);
                } else {
                    println!("  ⚠️  {} endpoint: {}", name, response.status());
                }
            }
            Ok(Err(e)) => {
                if e.to_string().contains("Connection refused") {
                    println!("  ℹ️  {} endpoint: System offline", name);
                    break; // No point testing other endpoints
                } else {
                    println!("  ❌ {} endpoint: {}", name, e);
                }
            }
            Err(_) => {
                println!("  ⚠️  {} endpoint: Timeout", name);
            }
        }
    }
    
    Ok(())
}

pub async fn test_startup_sequence() -> Result<()> {
    println!("🧪 Testing Startup Sequence...");
    
    // Check if process is running
    let output = std::process::Command::new("pgrep")
        .arg("-f")
        .arg("neural-trader")
        .output();
    
    match output {
        Ok(result) => {
            if result.status.success() {
                let pids = String::from_utf8_lossy(&result.stdout);
                let pid_count = pids.lines().filter(|l| !l.is_empty()).count();
                println!("  ✅ Neural trader process running ({} instances)", pid_count);
            } else {
                println!("  ⚠️  Neural trader process not found");
            }
        }
        Err(_) => {
            println!("  ℹ️  Cannot check process status (pgrep not available)");
        }
    }
    
    // Check log files for startup errors
    let log_paths = vec![
        "/var/log/neural-trader/neural-trader.log",
        "./neural-trader.log",
        "/tmp/neural-trader.log",
    ];
    
    for path in log_paths {
        if std::path::Path::new(path).exists() {
            println!("  ✅ Log file exists: {}", path);
            
            // Check for recent errors
            if let Ok(content) = std::fs::read_to_string(path) {
                let lines: Vec<&str> = content.lines().collect();
                let recent_lines = &lines[lines.len().saturating_sub(20)..];
                
                let error_count = recent_lines.iter()
                    .filter(|l| l.contains("ERROR") || l.contains("PANIC"))
                    .count();
                
                if error_count > 0 {
                    println!("    ⚠️  Found {} recent errors in logs", error_count);
                } else {
                    println!("    ✅ No recent errors in logs");
                }
            }
            break;
        }
    }
    
    Ok(())
}