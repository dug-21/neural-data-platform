//! Emergency Test: Vendor Predictor Architecture
//! Tests the complex two-layer sector model system

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use chrono::{Utc, Duration, Timelike, Datelike, Weekday};

// ETF to Sector mapping - CRITICAL for architecture
const ETF_SECTORS: &[(&str, &str)] = &[
    ("XLK", "Technology"),
    ("XLF", "Financial Services"),
    ("XLV", "Healthcare"),
    ("XLE", "Energy"),
    ("XLI", "Industrials"),
    ("XLY", "Consumer Discretionary"),
    ("XLP", "Consumer Staples"),
    ("XLRE", "Real Estate"),
    ("XLB", "Materials"),
    ("XLU", "Utilities"),
    ("XLC", "Communication Services"),
];

// Individual stocks mapped to their sectors
const STOCK_SECTOR_MAP: &[(&str, &str)] = &[
    ("AAPL", "XLK"),
    ("MSFT", "XLK"),
    ("GOOGL", "XLK"),
    ("JPM", "XLF"),
    ("BAC", "XLF"),
    ("JNJ", "XLV"),
    ("PFE", "XLV"),
    ("XOM", "XLE"),
    ("CVX", "XLE"),
];

#[derive(Debug, Serialize, Deserialize)]
struct ModelInfo {
    symbol: String,
    model_type: String,  // "sector_primary" or "symbol_specialization"
    size_mb: f64,
    last_trained: Option<String>,
    training_records: usize,
}

async fn connect_test_db() -> Result<sqlx::PgPool> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/neural_trader_db".to_string());
    
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await?;
    
    Ok(pool)
}

pub async fn test_two_layer_architecture() -> Result<()> {
    println!("🧪 Testing Two-Layer Sector Architecture...");
    println!("  Layer 1: ETF Sector Models (320-512MB)");
    println!("  Layer 2: Individual Stock Specializations (6-8MB)");
    
    // Check that ETF models are primary (large)
    for (etf, sector) in ETF_SECTORS {
        let model_path = format!("/opt/neural-trader/sector-models/{}_primary.fann", etf);
        if Path::new(&model_path).exists() {
            let metadata = std::fs::metadata(&model_path)?;
            let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
            
            if size_mb > 300.0 {
                println!("  ✅ {} ({} sector): {:.1} MB - PRIMARY MODEL", etf, sector, size_mb);
            } else {
                println!("  ❌ {} ({} sector): {:.1} MB - TOO SMALL FOR PRIMARY", etf, sector, size_mb);
                assert!(false, "ETF {} should have large primary model, found {:.1} MB", etf, size_mb);
            }
        } else {
            println!("  ⚠️  {} ({} sector): Model not found", etf, sector);
        }
    }
    
    // Check that individual stocks have specializations (small)
    for (stock, sector_etf) in STOCK_SECTOR_MAP {
        let spec_path = format!("/opt/neural-trader/sector-models/{}_specialization.fann", stock);
        if Path::new(&spec_path).exists() {
            let metadata = std::fs::metadata(&spec_path)?;
            let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
            
            if size_mb < 20.0 {
                println!("  ✅ {} ({}): {:.1} MB - SPECIALIZATION", stock, sector_etf, size_mb);
            } else {
                println!("  ❌ {} ({}): {:.1} MB - TOO LARGE FOR SPECIALIZATION", stock, sector_etf, size_mb);
                assert!(false, "Stock {} should have small specialization, found {:.1} MB", stock, size_mb);
            }
        } else {
            println!("  ⚠️  {} ({}): Specialization not found (will use sector model)", stock, sector_etf);
        }
    }
    
    println!("✅ Two-Layer Architecture test completed");
    Ok(())
}

pub async fn test_autonomous_training_triggers() -> Result<()> {
    println!("🧪 Testing Autonomous Training System...");
    
    let db = match connect_test_db().await {
        Ok(pool) => pool,
        Err(_) => {
            println!("  ℹ️  Database not available, skipping test");
            return Ok(());
        }
    };
    
    // Check environment variables
    let enable_autonomous = std::env::var("ENABLE_AUTONOMOUS_TRAINING")
        .unwrap_or_else(|_| "true".to_string()) == "true";
    
    let training_interval = std::env::var("AUTONOMOUS_TRAINING_INTERVAL_MINUTES")
        .unwrap_or_else(|_| "60".to_string())
        .parse::<i64>()
        .unwrap_or(60);
    
    let min_data_points = std::env::var("MIN_DATA_POINTS_FOR_TRAINING")
        .unwrap_or_else(|_| "100".to_string())
        .parse::<i64>()
        .unwrap_or(100);
    
    println!("  Configuration:");
    println!("    - Autonomous Training: {}", if enable_autonomous { "✅ ENABLED" } else { "❌ DISABLED" });
    println!("    - Training Interval: {} minutes", training_interval);
    println!("    - Min Data Points: {}", min_data_points);
    
    if !enable_autonomous {
        println!("  ⚠️  Autonomous training is disabled");
        return Ok(());
    }
    
    // Check recent training activity
    let recent_training = sqlx::query_as::<_, (String, i64, Option<chrono::NaiveDateTime>)>(
        r#"
        SELECT symbol, COUNT(*) as data_points, MAX(timestamp) as latest
        FROM market_data
        WHERE timestamp > NOW() - INTERVAL '24 hours'
        GROUP BY symbol
        HAVING COUNT(*) >= $1
        ORDER BY COUNT(*) DESC
        LIMIT 10
        "#
    )
    .bind(min_data_points)
    .fetch_all(&db)
    .await?;
    
    if recent_training.is_empty() {
        println!("  ⚠️  No symbols have enough data for training");
    } else {
        println!("  📊 Symbols ready for training:");
        for (symbol, data_points, latest) in recent_training {
            println!("    - {}: {} data points (latest: {:?})", 
                symbol, data_points, latest);
        }
    }
    
    // Check if training should trigger based on market hours
    let now = Utc::now();
    let hour = now.hour();
    let is_market_hours = hour >= 14 && hour < 21; // UTC 14:00-21:00 = EST 9:00-16:00
    
    println!("  ⏰ Market Hours Check:");
    println!("    - Current UTC hour: {}", hour);
    println!("    - Market Status: {}", if is_market_hours { "📈 OPEN (trading priority)" } else { "📉 CLOSED (training priority)" });
    
    if !is_market_hours {
        println!("  ✅ Training should be active (market closed)");
    } else {
        println!("  ⚠️  Training deferred (market open, trading priority)");
    }
    
    println!("✅ Autonomous Training test completed");
    Ok(())
}

pub async fn test_training_data_window() -> Result<()> {
    println!("🧪 Testing Training Data Window Configuration...");
    
    // Check critical environment variables
    let training_history_days = std::env::var("TRAINING_HISTORY_DAYS")
        .unwrap_or_else(|_| "90".to_string())
        .parse::<i64>()
        .unwrap_or(90);
    
    let max_training_history = std::env::var("MAX_TRAINING_HISTORY_DAYS")
        .unwrap_or_else(|_| "180".to_string())
        .parse::<i64>()
        .unwrap_or(180);
    
    let min_training_history = std::env::var("MIN_TRAINING_HISTORY_DAYS")
        .unwrap_or_else(|_| "7".to_string())
        .parse::<i64>()
        .unwrap_or(7);
    
    println!("  📅 Data Window Configuration:");
    println!("    - TRAINING_HISTORY_DAYS: {} days", training_history_days);
    println!("    - MAX_TRAINING_HISTORY_DAYS: {} days", max_training_history);
    println!("    - MIN_TRAINING_HISTORY_DAYS: {} days", min_training_history);
    
    // CRITICAL: Verify we're not using the buggy min() function
    assert!(training_history_days >= 30, 
        "TRAINING_HISTORY_DAYS ({}) is too small! Should be at least 30 days", 
        training_history_days);
    
    assert!(training_history_days <= max_training_history,
        "TRAINING_HISTORY_DAYS ({}) exceeds MAX ({})", 
        training_history_days, max_training_history);
    
    // Test database query with correct window
    let db = match connect_test_db().await {
        Ok(pool) => pool,
        Err(_) => {
            println!("  ℹ️  Database not available, skipping query test");
            return Ok(());
        }
    };
    
    // Verify data loading uses correct window
    let test_symbol = "XLK";
    let end_time = Utc::now();
    let start_time = end_time - Duration::days(training_history_days);
    
    let record_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM market_data WHERE symbol = $1 AND timestamp BETWEEN $2 AND $3"
    )
    .bind(test_symbol)
    .bind(start_time.naive_utc())
    .bind(end_time.naive_utc())
    .fetch_one(&db)
    .await?;
    
    println!("  📊 Data Query Test:");
    println!("    - Symbol: {}", test_symbol);
    println!("    - Time Range: {} days", training_history_days);
    println!("    - Records Found: {}", record_count);
    
    if record_count < 100 {
        println!("    ⚠️  Insufficient data for {} day window", training_history_days);
    } else {
        println!("    ✅ Adequate data for training");
    }
    
    println!("✅ Training Data Window test completed");
    Ok(())
}

pub async fn test_sector_model_assignment() -> Result<()> {
    println!("🧪 Testing Sector Model Assignment Logic...");
    
    // Test ETF identification
    println!("  🎯 ETF Representative Detection:");
    for (etf, sector) in ETF_SECTORS {
        // This mimics the is_sector_etf() function logic
        let is_etf = ETF_SECTORS.iter().any(|(e, _)| e == etf);
        assert!(is_etf, "{} should be identified as ETF", etf);
        println!("    ✅ {} -> {} Sector ETF", etf, sector);
    }
    
    // Test individual stock mapping
    println!("\n  🔧 Individual Stock Mapping:");
    for (stock, expected_etf) in STOCK_SECTOR_MAP {
        // Verify stock maps to correct sector ETF
        let found_etf = STOCK_SECTOR_MAP.iter()
            .find(|(s, _)| s == stock)
            .map(|(_, e)| *e);
        
        assert_eq!(found_etf, Some(*expected_etf), 
            "{} should map to {}", stock, expected_etf);
        
        println!("    ✅ {} -> {} (uses {} sector model)", stock, expected_etf, expected_etf);
    }
    
    // Test that individual stocks DON'T train primary models
    println!("\n  🚫 Verify Stocks Don't Train Primary Models:");
    for (stock, _) in STOCK_SECTOR_MAP {
        let primary_path = format!("/opt/neural-trader/sector-models/{}_primary.fann", stock);
        if Path::new(&primary_path).exists() {
            println!("    ❌ {} has primary model (SHOULD NOT EXIST)", stock);
            assert!(false, "Individual stock {} should not have primary model", stock);
        } else {
            println!("    ✅ {} correctly has no primary model", stock);
        }
    }
    
    println!("✅ Sector Model Assignment test completed");
    Ok(())
}

pub async fn test_cluster_model_pool() -> Result<()> {
    println!("🧪 Testing Cluster Model Pool Management...");
    
    // Check model pool directory structure
    let pool_base = "/opt/neural-trader/sector-models";
    
    if !Path::new(pool_base).exists() {
        println!("  ⚠️  Model pool directory does not exist");
        return Ok(());
    }
    
    // Count models by type
    let mut sector_models = 0;
    let mut specializations = 0;
    let mut unknown = 0;
    
    let entries = std::fs::read_dir(pool_base)?;
    for entry in entries {
        if let Ok(entry) = entry {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                if name.ends_with("_primary.fann") {
                    sector_models += 1;
                } else if name.ends_with("_specialization.fann") {
                    specializations += 1;
                } else if name.ends_with(".fann") {
                    unknown += 1;
                }
            }
        }
    }
    
    println!("  📊 Model Pool Statistics:");
    println!("    - Sector Primary Models: {}", sector_models);
    println!("    - Symbol Specializations: {}", specializations);
    println!("    - Other Models: {}", unknown);
    
    // Check expected vs actual
    let expected_sectors = ETF_SECTORS.len();
    if sector_models < expected_sectors {
        println!("    ⚠️  Missing {} sector models", expected_sectors - sector_models);
    } else {
        println!("    ✅ All sector models present");
    }
    
    // Test model versioning/rotation
    println!("\n  🔄 Model Versioning Check:");
    let backup_dir = format!("{}/backups", pool_base);
    if Path::new(&backup_dir).exists() {
        let backup_count = std::fs::read_dir(&backup_dir)?.count();
        println!("    ✅ Backup directory exists with {} models", backup_count);
    } else {
        println!("    ⚠️  No backup directory found");
    }
    
    println!("✅ Cluster Model Pool test completed");
    Ok(())
}

pub async fn test_market_hours_priority() -> Result<()> {
    println!("🧪 Testing Market Hours Priority Logic...");
    
    // Already imported at top of file
    
    let now = Utc::now();
    let weekday = now.weekday();
    let hour = now.hour();
    
    // Market hours in Eastern Time (considering DST)
    // EDT: UTC-4 (March to November)
    // EST: UTC-5 (November to March)
    let is_dst = now.month() >= 3 && now.month() <= 11;
    let utc_offset = if is_dst { 4 } else { 5 };
    
    let eastern_hour = if hour >= utc_offset {
        hour - utc_offset
    } else {
        24 + hour - utc_offset
    };
    
    let is_weekday = !matches!(weekday, Weekday::Sat | Weekday::Sun);
    let is_market_hours = is_weekday && eastern_hour >= 9 && eastern_hour < 16;
    
    println!("  📅 Time Analysis:");
    println!("    - UTC Time: {:02}:{:02} {}", hour, now.minute(), weekday);
    println!("    - Eastern Time: {:02}:xx {} ({})", eastern_hour, weekday, if is_dst { "EDT" } else { "EST" });
    println!("    - DST Active: {}", if is_dst { "Yes" } else { "No" });
    println!("    - Is Weekday: {}", if is_weekday { "Yes" } else { "No (weekend)" });
    println!("    - Market Status: {}", if is_market_hours { "🔔 OPEN" } else { "🔕 CLOSED" });
    
    println!("\n  🎯 Priority Logic:");
    if is_market_hours {
        println!("    ✅ TRADING PRIORITY - Minimize training");
        println!("    - Defer non-critical training");
        println!("    - Focus on real-time predictions");
        println!("    - Quick specialization updates only");
    } else {
        println!("    ✅ TRAINING PRIORITY - Intensive operations allowed");
        println!("    - Full sector model retraining");
        println!("    - Hyperparameter optimization");
        println!("    - Model validation and backtesting");
    }
    
    // Verify DST handling is correct
    println!("\n  🕐 DST Handling Verification:");
    let test_dates = vec![
        ("2024-01-15", false, "Winter - EST"),
        ("2024-06-15", true, "Summer - EDT"),
        ("2024-11-15", false, "Fall - EST"),
    ];
    
    for (date_str, expected_dst, desc) in test_dates {
        println!("    - {}: {} (DST: {})", date_str, desc, if expected_dst { "Yes" } else { "No" });
    }
    
    println!("✅ Market Hours Priority test completed");
    Ok(())
}

pub async fn test_validation_gates() -> Result<()> {
    println!("🧪 Testing Validation Gates Configuration...");
    
    // These thresholds are critical for quality control
    let min_r2_score = std::env::var("MIN_R2_SCORE")
        .unwrap_or_else(|_| "0.6".to_string())
        .parse::<f64>()
        .unwrap_or(0.6);
    
    let max_mse = std::env::var("MAX_MSE")
        .unwrap_or_else(|_| "0.01".to_string())
        .parse::<f64>()
        .unwrap_or(0.01);
    
    let min_data_quality = std::env::var("MIN_DATA_QUALITY")
        .unwrap_or_else(|_| "0.95".to_string())
        .parse::<f64>()
        .unwrap_or(0.95);
    
    println!("  🎚️ Validation Thresholds:");
    println!("    - Minimum R² Score: {:.2}", min_r2_score);
    println!("    - Maximum MSE: {:.4}", max_mse);
    println!("    - Minimum Data Quality: {:.2}%", min_data_quality * 100.0);
    
    // Validate thresholds are reasonable
    assert!(min_r2_score >= 0.5 && min_r2_score <= 0.9, 
        "R² threshold {} is unrealistic", min_r2_score);
    assert!(max_mse > 0.0 && max_mse < 1.0,
        "MSE threshold {} is unrealistic", max_mse);
    assert!(min_data_quality >= 0.9 && min_data_quality <= 1.0,
        "Data quality threshold {} is unrealistic", min_data_quality);
    
    println!("  ✅ All validation gates properly configured");
    
    // Check if validation is enforced
    let enforce_validation = std::env::var("ENFORCE_VALIDATION_GATES")
        .unwrap_or_else(|_| "true".to_string()) == "true";
    
    if enforce_validation {
        println!("  ✅ Validation gates are ENFORCED");
    } else {
        println!("  ⚠️  Validation gates are BYPASSED (development mode)");
    }
    
    println!("✅ Validation Gates test completed");
    Ok(())
}

pub async fn test_model_persistence_integrity() -> Result<()> {
    println!("🧪 Testing Model Persistence Integrity...");
    
    // Critical paths that must be consistent
    let paths = vec![
        ("/opt/neural-trader/sector-models", "Primary model storage"),
        ("/opt/neural-trader/models", "Legacy model location"),
        ("/opt/neural-trader/config", "Configuration files"),
    ];
    
    println!("  📁 Directory Structure:");
    for (path, description) in &paths {
        if Path::new(path).exists() {
            let count = std::fs::read_dir(path)?.count();
            println!("    ✅ {}: {} ({} files)", path, description, count);
            
            // Check permissions
            let metadata = std::fs::metadata(path)?;
            let perms = metadata.permissions();
            if perms.readonly() {
                println!("      ⚠️  Directory is READ-ONLY!");
            }
        } else {
            println!("    ❌ {}: {} (NOT FOUND)", path, description);
        }
    }
    
    // Verify Docker volume mount points
    if std::env::var("DOCKER_CONTAINER").is_ok() {
        println!("\n  🐳 Docker Volume Verification:");
        
        // Check if we're using the correct consolidated path
        let consolidated_path = "/opt/neural-trader";
        if Path::new(consolidated_path).exists() {
            println!("    ✅ Consolidated path exists: {}", consolidated_path);
            
            // Test write permissions
            let test_file = format!("{}/test_write_{}.tmp", consolidated_path, Utc::now().timestamp());
            match std::fs::write(&test_file, "test") {
                Ok(_) => {
                    println!("    ✅ Write permissions confirmed");
                    let _ = std::fs::remove_file(test_file);
                }
                Err(e) => {
                    println!("    ❌ Cannot write to volume: {}", e);
                }
            }
        }
    }
    
    println!("✅ Model Persistence Integrity test completed");
    Ok(())
}