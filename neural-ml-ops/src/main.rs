//! Neural ML-Ops - Domain Agnostic ML Operations Platform
//!
//! This binary provides a complete ML operations platform with:
//! - Training coordination and scheduling
//! - Feature engineering and storage
//! - Model registry and versioning  
//! - Event publishing for ML workflows
//!
//! Extracted from trading-specific code and made domain agnostic.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

mod training;
mod features;
mod models;
mod events;

use training::TrainingCoordinator;
use features::FeatureStoreConfig;
use models::ModelRegistryConfig;
use events::EventPublisher;

#[derive(Parser)]
#[command(name = "neural-ml-ops")]
#[command(about = "Domain-agnostic ML Operations platform")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    /// Configuration file path
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,
    
    /// Log level
    #[arg(short, long, default_value = "info")]
    log_level: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the ML-Ops coordinator
    Start {
        /// Bind address
        #[arg(short, long, default_value = "127.0.0.1:8080")]
        bind: String,
    },
    /// Run training workflow
    Train {
        /// Workflow name
        workflow: String,
        /// Input data path
        #[arg(short, long)]
        data: PathBuf,
        /// Output model path
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Feature engineering pipeline
    Features {
        /// Input data path
        input: PathBuf,
        /// Output features path
        output: PathBuf,
        /// Feature configuration
        #[arg(short, long)]
        config: Option<PathBuf>,
    },
    /// Model management
    Models {
        #[command(subcommand)]
        action: ModelAction,
    },
}

#[derive(Subcommand)]
enum ModelAction {
    /// List models in registry
    List,
    /// Show model details
    Info { model_id: String },
    /// Export model
    Export { 
        model_id: String,
        output: PathBuf,
    },
    /// Import model
    Import {
        path: PathBuf,
        model_id: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(&cli.log_level)
        .init();
    
    info!("Starting Neural ML-Ops v{}", env!("CARGO_PKG_VERSION"));
    
    // Load configuration
    let config = load_config(&cli.config).await?;
    
    match cli.command {
        Commands::Start { bind } => start_coordinator(&bind, config).await,
        Commands::Train { workflow, data, output } => {
            run_training_workflow(&workflow, &data, &output, config).await
        },
        Commands::Features { input, output, config: feature_config } => {
            run_feature_pipeline(&input, &output, feature_config).await
        },
        Commands::Models { action } => {
            handle_model_action(action, config).await
        },
    }
}

/// Load configuration from file
async fn load_config(path: &PathBuf) -> Result<MLOpsConfig> {
    if path.exists() {
        info!("Loading configuration from {}", path.display());
        let content = tokio::fs::read_to_string(path).await?;
        Ok(toml::from_str(&content)?)
    } else {
        warn!("Configuration file not found, using defaults");
        Ok(MLOpsConfig::default())
    }
}

/// Start the ML-Ops coordinator service
async fn start_coordinator(bind: &str, config: MLOpsConfig) -> Result<()> {
    info!("Starting ML-Ops coordinator on {}", bind);
    
    let coordinator = TrainingCoordinator::new(config.training).await?;
    let feature_store = features::FeatureStore::new(config.features).await?;
    let model_registry = models::ModelRegistry::new(config.models).await?;
    let event_publisher = EventPublisher::new(config.events).await?;
    
    // Create application state
    let app_state = AppState {
        coordinator: Arc::new(coordinator),
        feature_store: Arc::new(feature_store),
        model_registry: Arc::new(model_registry),
        event_publisher: Arc::new(event_publisher),
    };
    
    // Build the web server
    let app = create_routes(app_state);
    
    // Start server
    let listener = tokio::net::TcpListener::bind(bind).await?;
    info!("Neural ML-Ops coordinator listening on {}", bind);
    
    axum::serve(listener, app).await?;
    
    Ok(())
}

/// Run a training workflow
async fn run_training_workflow(
    workflow: &str,
    data_path: &PathBuf,
    output_path: &PathBuf,
    config: MLOpsConfig,
) -> Result<()> {
    info!("Running training workflow: {}", workflow);
    info!("Data path: {}", data_path.display());
    info!("Output path: {}", output_path.display());
    
    let coordinator = TrainingCoordinator::new(config.training).await?;
    
    // Execute workflow
    let workflow_config = coordinator.get_workflow_config(workflow)?;
    let result = coordinator.execute_workflow(&workflow_config, data_path, output_path).await?;
    
    info!("Training completed: {:?}", result);
    Ok(())
}

/// Run feature engineering pipeline
async fn run_feature_pipeline(
    input: &PathBuf,
    output: &PathBuf,
    feature_config: Option<PathBuf>,
) -> Result<()> {
    info!("Running feature pipeline");
    info!("Input: {}", input.display());
    info!("Output: {}", output.display());
    
    // Load feature configuration
    let config = if let Some(config_path) = feature_config {
        let content = tokio::fs::read_to_string(&config_path).await?;
        serde_json::from_str(&content)?
    } else {
        features::FeatureConfig::default()
    };
    
    let feature_engine = features::FeatureEngine::new(config);
    
    // Process data
    let input_data = load_training_data(input).await?;
    let features = feature_engine.extract_features(&input_data).await?;
    
    // Save features
    save_features(&features, output).await?;
    
    info!("Feature pipeline completed, {} features extracted", features.len());
    Ok(())
}

/// Handle model management actions
async fn handle_model_action(action: ModelAction, config: MLOpsConfig) -> Result<()> {
    let model_registry = models::ModelRegistry::new(config.models).await?;
    
    match action {
        ModelAction::List => {
            let models = model_registry.list_models(None).await?;
            println!("Available models:");
            for model in models {
                println!("  {} - {} (v{})", model.id, model.name, model.version);
            }
        },
        ModelAction::Info { model_id } => {
            let model_info = model_registry.get_model_info(&model_id).await?;
            println!("Model Information:");
            println!("  ID: {}", model_info.id);
            println!("  Name: {}", model_info.name);
            println!("  Version: {}", model_info.version);
            println!("  Created: {}", model_info.created_at);
            println!("  Metrics: {:?}", model_info.metrics);
        },
        ModelAction::Export { model_id, output } => {
            model_registry.export_model(&model_id, &output).await?;
            info!("Model {} exported to {}", model_id, output.display());
        },
        ModelAction::Import { path, model_id } => {
            model_registry.import_model(&path, &model_id).await?;
            info!("Model {} imported from {}", model_id, path.display());
        },
    }
    
    Ok(())
}

/// Application configuration
#[derive(Debug, Clone, serde::Deserialize)]
struct MLOpsConfig {
    #[serde(default)]
    training: training::TrainingConfig,
    #[serde(default)]
    features: FeatureStoreConfig,
    #[serde(default)]
    models: ModelRegistryConfig,
    #[serde(default)]
    events: events::EventConfig,
}

impl Default for MLOpsConfig {
    fn default() -> Self {
        Self {
            training: training::TrainingConfig::default(),
            features: FeatureStoreConfig::default(),
            models: ModelRegistryConfig::default(),
            events: events::EventConfig::default(),
        }
    }
}

/// Application state shared across handlers
#[derive(Clone)]
struct AppState {
    coordinator: Arc<TrainingCoordinator>,
    feature_store: Arc<features::FeatureStore>,
    model_registry: Arc<models::ModelRegistry>,
    event_publisher: Arc<EventPublisher>,
}

/// Create HTTP routes
fn create_routes(state: AppState) -> axum::Router {
    use axum::{
        routing::{get, post},
        Router,
    };
    
    Router::new()
        .route("/health", get(health_check))
        .route("/training/workflows", get(list_workflows))
        .route("/training/workflows/:id/start", post(start_workflow))
        .route("/training/status/:id", get(get_training_status))
        .route("/features/extract", post(extract_features))
        .route("/models", get(list_models))
        .route("/models/:id", get(get_model))
        .route("/events/publish", post(publish_event))
        .with_state(state)
}

// HTTP handlers
async fn health_check() -> &'static str {
    "Neural ML-Ops is running"
}

async fn list_workflows(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<axum::Json<Vec<String>>, axum::http::StatusCode> {
    let workflows = state.coordinator.list_workflows().await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(axum::Json(workflows))
}

async fn start_workflow(
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Json(payload): axum::extract::Json<serde_json::Value>,
) -> Result<axum::Json<String>, axum::http::StatusCode> {
    let workflow_id = state.coordinator.start_workflow(&id, payload).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(axum::Json(workflow_id))
}

async fn get_training_status(
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<axum::Json<training::TrainingStatus>, axum::http::StatusCode> {
    let status = state.coordinator.get_status(&id).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(axum::Json(status))
}

async fn extract_features(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Json(request): axum::extract::Json<features::FeatureRequest>,
) -> Result<axum::Json<Vec<features::Feature>>, axum::http::StatusCode> {
    let features = state.feature_store.extract_features(request).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(axum::Json(features))
}

async fn list_models(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<axum::Json<Vec<models::ModelInfo>>, axum::http::StatusCode> {
    let models = state.model_registry.list_models(None).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(axum::Json(models))
}

async fn get_model(
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<axum::Json<models::ModelInfo>, axum::http::StatusCode> {
    let model = state.model_registry.get_model_info(&id).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(axum::Json(model))
}

async fn publish_event(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Json(event): axum::extract::Json<events::MLEvent>,
) -> Result<&'static str, axum::http::StatusCode> {
    state.event_publisher.publish(event).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok("Event published")
}

// Helper functions
async fn load_training_data(path: &PathBuf) -> Result<Vec<f64>> {
    let content = tokio::fs::read_to_string(path).await?;
    let data: Vec<f64> = content.lines()
        .map(|line| line.parse())
        .collect::<Result<Vec<_>, _>>()?;
    Ok(data)
}

async fn save_features(features: &[features::Feature], path: &PathBuf) -> Result<()> {
    let json = serde_json::to_string_pretty(features)?;
    tokio::fs::write(path, json).await?;
    Ok(())
}