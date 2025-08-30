//! MVP Neural Network Trainer CLI
//!
//! Command-line tool for training and validating the MVP neural network model
//! Provides comprehensive training pipeline with validation and reporting

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{info, warn, error};
use std::collections::HashMap;

use autonomous_platform::data::TimeSeriesData;
use autonomous_platform::neural::mvp_predictor::{MVPPredictor, MVPPredictorBuilder, SimpleDecisionLogic};
use autonomous_platform::integration::mvp_training_service::{
    MVPTrainingService, MVPDataRequirements, MVPTrainingResult
};
use autonomous_platform::backtesting::mvp_backtester::{MVPBacktester, BacktestConfig, BacktestResult};
use autonomous_platform::adapters::model_storage::ModelStorageConfig;
use autonomous_platform::adapters::timescale::TimescaleAdapter;

/// MVP Neural Network Trainer
#[derive(Parser)]
#[command(name = "mvp-trainer")]
#[command(about = "MVP Neural Network Trainer for Market Prediction")]
#[command(version = "1.0.0")]
struct Args {
    #[command(subcommand)]
    command: Commands,
    
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
    
    /// Model storage directory
    #[arg(short = 's', long, default_value = "./models")]
    storage_path: PathBuf,
    
    /// Database connection URL
    #[arg(short = 'd', long, env = "DATABASE_URL")]
    database_url: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Train a new MVP model
    Train {
        /// Symbol to train on (e.g., AAPL, SPY)
        symbol: String,
        
        /// Model name
        #[arg(short, long, default_value = "mvp_model")]
        model_name: String,
        
        /// Minimum training samples required
        #[arg(long, default_value = "1000")]
        min_samples: usize,
        
        /// Validation split ratio (0.0 to 1.0)
        #[arg(long, default_value = "0.2")]
        validation_split: f32,
        
        /// Skip data availability check
        #[arg(long)]
        skip_check: bool,
    },
    
    /// Validate existing model with backtesting
    Test {
        /// Model name to test
        model_name: String,
        
        /// Symbol to test on
        symbol: String,
        
        /// Test period in days
        #[arg(long, default_value = "252")]
        test_days: usize,
        
        /// Initial capital for backtesting
        #[arg(long, default_value = "100000")]
        capital: f64,
        
        /// Generate detailed report
        #[arg(long)]
        detailed: bool,
    },
    
    /// Check data availability for training
    Check {
        /// Symbol to check
        symbol: String,
        
        /// Required days of data
        #[arg(long, default_value = "1000")]
        required_days: usize,
    },
    
    /// List trained models
    List {
        /// Show detailed model information
        #[arg(long)]
        detailed: bool,
    },
    
    /// Benchmark model against buy-and-hold strategy
    Benchmark {
        /// Model name to benchmark
        model_name: String,
        
        /// Symbol to benchmark on
        symbol: String,
        
        /// Benchmark period in days
        #[arg(long, default_value = "252")]
        period_days: usize,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    // Initialize logging
    let log_level = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(format!("mvp_trainer={},autonomous_platform={}", log_level, log_level))
        .init();
    
    info!("🚀 MVP Neural Network Trainer v1.0.0");
    
    match args.command {
        Commands::Train { symbol, model_name, min_samples, validation_split, skip_check } => {
            train_model(args, symbol, model_name, min_samples, validation_split, skip_check).await?;
        }
        Commands::Test { model_name, symbol, test_days, capital, detailed } => {
            test_model(args, model_name, symbol, test_days, capital, detailed).await?;
        }
        Commands::Check { symbol, required_days } => {
            check_data_availability(args, symbol, required_days).await?;
        }
        Commands::List { detailed } => {
            list_models(args, detailed).await?;
        }
        Commands::Benchmark { model_name, symbol, period_days } => {
            benchmark_model(args, model_name, symbol, period_days).await?;
        }
    }
    
    Ok(())
}

/// Train a new MVP neural network model
async fn train_model(
    args: Args,
    symbol: String,
    model_name: String,
    min_samples: usize,
    validation_split: f32,
    skip_check: bool,
) -> Result<()> {
    info!("🎯 Training MVP model '{}' on symbol {}", model_name, symbol);
    
    // Set up storage configuration
    let storage_config = ModelStorageConfig {
        base_path: args.storage_path.clone(),
        ..Default::default()
    };
    
    // Set up database connection
    let database_url = args.database_url
        .or_else(|| std::env::var("DATABASE_URL").ok())
        .ok_or_else(|| anyhow!("Database URL required (set DATABASE_URL or use --database-url)"))?;
    
    let timescale_adapter = TimescaleAdapter::new(&database_url).await?;
    
    // Configure training requirements
    let requirements = MVPDataRequirements {
        min_training_samples: min_samples,
        validation_split,
        input_window_days: 20,
        prediction_horizon_days: 1,
        symbol: symbol.clone(),
    };
    
    // Create training service
    let training_service = MVPTrainingService::new(timescale_adapter, requirements.clone());
    
    // Check data availability unless skipped
    if !skip_check {
        info!("📊 Checking data availability...");
        let has_data = training_service.validate_data_availability().await?;
        if !has_data {
            error!("❌ Insufficient data available for training");
            return Err(anyhow!("Not enough historical data for symbol {}", symbol));
        }
        info!("✅ Data availability check passed");
    }
    
    // Create MVP predictor
    let decision_logic = SimpleDecisionLogic {
        buy_threshold: 0.02,   // 2% expected return
        sell_threshold: -0.02, // -2% expected loss
        min_confidence: 0.6,   // 60% minimum confidence
    };
    
    let mut predictor = MVPPredictorBuilder::new(model_name.clone())
        .with_storage_config(storage_config)
        .with_decision_logic(decision_logic)
        .build()
        .await?;
    
    // Train the model
    info!("🏋️ Starting training process...");
    let training_result = training_service.train_model(&mut predictor).await?;
    
    // Display results
    display_training_results(&training_result);
    
    if training_result.success {
        info!("✅ Training completed successfully!");
        info!("💾 Model saved and ready for use");
    } else {
        warn!("⚠️ Training completed but did not meet success criteria");
        warn!("   Consider adjusting parameters or collecting more data");
    }
    
    Ok(())
}

/// Test existing model with backtesting
async fn test_model(
    args: Args,
    model_name: String,
    symbol: String,
    test_days: usize,
    capital: f64,
    detailed: bool,
) -> Result<()> {
    info!("🧪 Testing model '{}' on {} ({} days)", model_name, symbol, test_days);
    
    // Set up storage configuration
    let storage_config = ModelStorageConfig {
        base_path: args.storage_path,
        ..Default::default()
    };
    
    // Load existing predictor
    let predictor = MVPPredictorBuilder::new(model_name.clone())
        .with_storage_config(storage_config)
        .build()
        .await?;
    
    if !predictor.is_ready() {
        return Err(anyhow!("Model '{}' is not trained or ready", model_name));
    }
    
    // Set up database connection for test data
    let database_url = args.database_url
        .or_else(|| std::env::var("DATABASE_URL").ok())
        .ok_or_else(|| anyhow!("Database URL required"))?;
    
    let timescale_adapter = TimescaleAdapter::new(&database_url).await?;
    
    // Load test data
    let end_date = chrono::Utc::now();
    let start_date = end_date - chrono::Duration::days(test_days as i64 + 30); // Extra buffer for features
    
    let historical_data = timescale_adapter
        .get_historical_data(&symbol, start_date, end_date)
        .await?;
    
    if historical_data.len() < test_days {
        return Err(anyhow!("Insufficient test data: need {} days, got {}", test_days, historical_data.len()));
    }
    
    info!("📈 Loaded {} days of test data", historical_data.len());
    
    // Generate predictions for backtesting
    let mut predictions = Vec::new();
    let mut actual_prices = Vec::new();
    let mut timestamps = Vec::new();
    
    // Use sliding window to generate predictions
    let window_start = 30; // Allow for feature calculation
    for i in window_start..historical_data.len() {
        let window_data = &historical_data[i-30..i]; // 30-day window for features
        
        match predictor.predict(window_data).await {
            Ok(prediction) => {
                predictions.push(prediction);
                actual_prices.push(historical_data[i].close as f64);
                timestamps.push(historical_data[i].timestamp);
            }
            Err(e) => {
                warn!("⚠️ Prediction failed for day {}: {}", i, e);
            }
        }
    }
    
    if predictions.is_empty() {
        return Err(anyhow!("No predictions generated"));
    }
    
    info!("🔮 Generated {} predictions", predictions.len());
    
    // Run backtest
    let backtest_config = BacktestConfig {
        initial_capital: capital,
        transaction_cost: 0.001, // 0.1%
        position_size: 0.1,      // 10%
        max_positions: 1,
        risk_free_rate: 0.02,    // 2% annual
    };
    
    let mut backtester = MVPBacktester::new(backtest_config);
    let backtest_result = backtester.run_backtest(&predictions, &actual_prices, &timestamps)?;
    
    // Display results
    display_backtest_results(&backtest_result, detailed);
    
    Ok(())
}

/// Check data availability for training
async fn check_data_availability(args: Args, symbol: String, required_days: usize) -> Result<()> {
    info!("📊 Checking data availability for {} ({} days required)", symbol, required_days);
    
    let database_url = args.database_url
        .or_else(|| std::env::var("DATABASE_URL").ok())
        .ok_or_else(|| anyhow!("Database URL required"))?;
    
    let timescale_adapter = TimescaleAdapter::new(&database_url).await?;
    
    let end_date = chrono::Utc::now();
    let start_date = end_date - chrono::Duration::days((required_days + 100) as i64);
    
    let data_count = timescale_adapter.count_data_points(&symbol, start_date, end_date).await?;
    
    info!("📈 Available data points: {}", data_count);
    info!("📋 Required data points: {}", required_days);
    
    if data_count >= required_days {
        info!("✅ Sufficient data available for training");
    } else {
        warn!("❌ Insufficient data: need {} more days", required_days - data_count);
    }
    
    Ok(())
}

/// List trained models
async fn list_models(args: Args, detailed: bool) -> Result<()> {
    info!("📋 Listing trained models in {:?}", args.storage_path);
    
    // Simple implementation - just list directories in storage path
    if !args.storage_path.exists() {
        warn!("📁 Storage directory does not exist: {:?}", args.storage_path);
        return Ok(());
    }
    
    let mut model_count = 0;
    
    for entry in std::fs::read_dir(&args.storage_path)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_dir() {
            let model_name = path.file_name().unwrap().to_string_lossy();
            model_count += 1;
            
            if detailed {
                info!("📦 Model: {}", model_name);
                
                // Try to find model metadata
                let metadata_path = path.join("metadata.json");
                if metadata_path.exists() {
                    match std::fs::read_to_string(&metadata_path) {
                        Ok(content) => {
                            if let Ok(metadata) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(version) = metadata.get("version") {
                                    info!("   Version: {}", version);
                                }
                                if let Some(timestamp) = metadata.get("timestamp") {
                                    info!("   Created: {}", timestamp);
                                }
                                if let Some(accuracy) = metadata.get("accuracy") {
                                    info!("   Accuracy: {}", accuracy);
                                }
                            }
                        }
                        Err(_) => info!("   (metadata unavailable)"),
                    }
                } else {
                    info!("   (no metadata found)");
                }
            } else {
                info!("📦 {}", model_name);
            }
        }
    }
    
    if model_count == 0 {
        info!("📭 No trained models found");
    } else {
        info!("📊 Total models: {}", model_count);
    }
    
    Ok(())
}

/// Benchmark model against buy-and-hold
async fn benchmark_model(
    args: Args,
    model_name: String,
    symbol: String,
    period_days: usize,
) -> Result<()> {
    info!("🏁 Benchmarking model '{}' vs buy-and-hold on {} ({} days)", model_name, symbol, period_days);
    
    // This would involve:
    // 1. Running the model's backtest
    // 2. Running a buy-and-hold backtest
    // 3. Comparing the results
    // 4. Displaying comparative metrics
    
    info!("⚠️ Benchmark functionality not yet implemented in MVP");
    info!("   Use 'test' command to evaluate model performance");
    
    Ok(())
}

/// Display training results in a formatted way
fn display_training_results(result: &MVPTrainingResult) {
    println!("\n🎯 Training Results for {}", result.symbol);
    println!("=" .repeat(50));
    
    // Data statistics
    println!("\n📊 Data Statistics:");
    println!("   Total samples: {}", result.data_stats.total_samples);
    println!("   Training samples: {}", result.data_stats.training_samples);
    println!("   Validation samples: {}", result.data_stats.validation_samples);
    println!("   Features: {}", result.data_stats.feature_count);
    println!("   Price range: ${:.2} - ${:.2}", result.data_stats.price_range.0, result.data_stats.price_range.1);
    println!("   Mean return: {:.4} ({:.2}%)", result.data_stats.mean_return, result.data_stats.mean_return * 100.0);
    println!("   Return std: {:.4} ({:.2}%)", result.data_stats.return_std, result.data_stats.return_std * 100.0);
    
    // Training results
    println!("\n🏋️ Training Results:");
    println!("   Epochs completed: {}", result.training_record.epochs_completed);
    println!("   Final MSE: {:.6}", result.training_record.final_mse);
    println!("   Training time: {} seconds", result.training_record.training_time_secs);
    
    // Validation metrics
    println!("\n🔍 Validation Metrics:");
    println!("   MSE: {:.6}", result.validation_metrics.mse);
    println!("   R²: {:.4} ({:.1}%)", result.validation_metrics.r_squared, result.validation_metrics.r_squared * 100.0);
    println!("   MAE: {:.6}", result.validation_metrics.mae);
    println!("   Direction Accuracy: {:.1}%", result.validation_metrics.direction_accuracy * 100.0);
    println!("   Sharpe Ratio: {:.2}", result.validation_metrics.sharpe_ratio);
    println!("   Max Error: {:.6}", result.validation_metrics.max_error);
    
    // Success criteria
    println!("\n✅ Success Criteria:");
    let r2_pass = result.validation_metrics.r_squared > 0.05;
    let dir_acc_pass = result.validation_metrics.direction_accuracy > 0.52;
    let mse_pass = result.training_record.final_mse < 0.01;
    
    println!("   R² > 0.05: {} ({:.4})", if r2_pass { "✅ PASS" } else { "❌ FAIL" }, result.validation_metrics.r_squared);
    println!("   Direction Accuracy > 52%: {} ({:.1}%)", if dir_acc_pass { "✅ PASS" } else { "❌ FAIL" }, result.validation_metrics.direction_accuracy * 100.0);
    println!("   Training MSE < 0.01: {} ({:.6})", if mse_pass { "✅ PASS" } else { "❌ FAIL" }, result.training_record.final_mse);
    
    println!("\n🏆 Overall Success: {}", if result.success { "✅ PASS" } else { "❌ FAIL" });
}

/// Display backtest results in a formatted way
fn display_backtest_results(result: &BacktestResult, detailed: bool) {
    println!("\n🧪 Backtest Results");
    println!("=" .repeat(50));
    
    // Performance summary
    println!("\n💰 Performance Summary:");
    println!("   Initial Capital: ${:,.0}", result.initial_capital);
    println!("   Final Capital: ${:,.0}", result.final_capital);
    println!("   Total Return: {:.2}%", result.total_return * 100.0);
    println!("   Annual Return: {:.2}%", result.annual_return * 100.0);
    println!("   Max Drawdown: {:.2}%", result.max_drawdown * 100.0);
    
    // Risk metrics
    println!("\n⚖️ Risk Metrics:");
    println!("   Sharpe Ratio: {:.2}", result.sharpe_ratio);
    println!("   Sortino Ratio: {:.2}", result.sortino_ratio);
    println!("   Calmar Ratio: {:.2}", result.calmar_ratio);
    println!("   Volatility: {:.2}%", result.volatility * 100.0);
    println!("   VaR 95%: {:.2}%", result.var_95 * 100.0);
    
    // Trade statistics
    println!("\n📊 Trade Statistics:");
    println!("   Total Trades: {}", result.total_trades);
    println!("   Win Rate: {:.1}%", result.win_rate * 100.0);
    println!("   Avg Win: ${:.2}", result.avg_win);
    println!("   Avg Loss: ${:.2}", result.avg_loss);
    println!("   Profit Factor: {:.2}", result.profit_factor);
    println!("   Avg Holding: {:.1} days", result.avg_holding_days);
    println!("   Max Consecutive Losses: {}", result.max_consecutive_losses);
    
    // Benchmark comparison
    println!("\n🏁 Benchmark Comparison:");
    println!("   Benchmark Return: {:.2}%", result.benchmark_return * 100.0);
    println!("   Alpha: {:.2}%", result.alpha * 100.0);
    println!("   Information Ratio: {:.2}", result.information_ratio);
    
    if detailed && !result.trades.is_empty() {
        println!("\n📋 Recent Trades (last 10):");
        for trade in result.trades.iter().rev().take(10) {
            println!("   {} {} shares @ ${:.2} → ${:.2} = ${:.2} ({:.1}% over {:.0} days)", 
                     trade.direction,
                     trade.shares,
                     trade.entry_price,
                     trade.exit_price,
                     trade.net_pnl,
                     trade.return_pct() * 100.0,
                     trade.holding_days);
        }
    }
}