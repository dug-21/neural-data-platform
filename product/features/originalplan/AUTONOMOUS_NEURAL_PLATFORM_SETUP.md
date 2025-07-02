# Autonomous Neural Platform Setup Guide

## Overview

This guide provides a complete framework for building autonomous decision-making systems using neural networks. The platform is designed to be domain-agnostic and can be adapted for trading, IoT control, recommendation systems, or any application requiring real-time intelligent decision-making.

## Architecture Principles

### Core Components
1. **Data Platform** - Dockerized time-series database and caching layer
2. **Neural Engine** - ruv-FANN based neural network processing
3. **Agent Layer** - ruv-DAA autonomous decision-making agents
4. **MCP Integration** - Model Context Protocol for AI coordination
5. **Data Connectors** - Dockerized microservices for external data sources

### Design Philosophy
- **Container-First**: Everything runs in Docker for portability
- **Real-Time Capable**: Sub-100ms decision latency
- **Horizontally Scalable**: Add agents and data sources independently
- **AI-Native**: Built for AI agent coordination via MCP
- **Memory Safe**: Rust-based with zero unsafe code

## Project Structure Template

```
autonomous-platform/
├── Cargo.toml                    # Root workspace configuration
├── docker-compose.yml            # Full platform orchestration
├── src/
│   ├── lib.rs                    # Platform core library
│   ├── main.rs                   # CLI entry point
│   ├── data/                     # Data platform core
│   │   ├── mod.rs
│   │   ├── ingestion.rs          # Data ingestion pipeline
│   │   ├── storage.rs            # TimescaleDB integration
│   │   ├── cache.rs              # Redis caching layer
│   │   └── quality.rs            # Data quality monitoring
│   ├── neural/                   # Neural processing layer
│   │   ├── mod.rs
│   │   ├── engine.rs             # ruv-FANN integration
│   │   ├── models.rs             # Neural model management
│   │   └── training.rs           # Online learning pipeline
│   ├── agents/                   # Autonomous agent layer
│   │   ├── mod.rs
│   │   ├── orchestrator.rs       # ruv-DAA integration
│   │   ├── base_agent.rs         # Agent trait definitions
│   │   └── registry.rs           # Agent lifecycle management
│   ├── mcp/                      # Model Context Protocol
│   │   ├── mod.rs
│   │   ├── server.rs             # MCP server implementation
│   │   ├── tools.rs              # Platform-specific tools
│   │   └── handlers.rs           # Message handling
│   └── config/                   # Configuration management
│       ├── mod.rs
│       ├── settings.rs           # Runtime settings
│       └── validation.rs         # Config validation
├── connectors/                   # Data connector microservices
│   ├── Dockerfile.template       # Template for new connectors
│   ├── common/                   # Shared connector utilities
│   └── examples/                 # Example connector implementations
├── docker/                       # Docker configurations
│   ├── data-platform/            # Database and cache setup
│   ├── neural-engine/            # Neural processing container
│   └── monitoring/               # Observability stack
├── tests/                        # Integration tests
├── examples/                     # Usage examples
└── docs/                         # Additional documentation
```

## Core Dependencies (Cargo.toml)

```toml
[package]
name = "autonomous-platform"
version = "0.1.0"
edition = "2021"

[workspace]
members = ["connectors/*"]

[dependencies]
# Core ruv ecosystem
ruv-fann = "0.1.3"
ruv-swarm-core = "0.2.0"
ruv-swarm-ml = "0.2.0"
ruv-swarm-mcp = "0.2.0"

# DAA integration
ruv-daa = { git = "https://github.com/ruvnet/daa.git", branch = "main" }

# Async runtime
tokio = { version = "1.39", features = ["full"] }
async-trait = "0.1"
futures = "0.3"

# Data handling
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "postgres", "chrono"] }
redis = { version = "0.25", features = ["tokio-comp"] }

# Time series
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.9", features = ["v4", "serde"] }

# Networking
axum = "0.7"
tower = "0.4"
hyper = "1.0"

# Configuration
config = "0.14"
toml = "0.8"

# Monitoring
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
metrics = "0.22"

# Error handling
anyhow = "1.0"
thiserror = "1.0"

[features]
default = ["std"]
std = []
gpu = ["ruv-fann/gpu"]
distributed = ["ruv-swarm-core/distributed"]
```

## Data Platform Setup

### 1. TimescaleDB Configuration

**docker/data-platform/docker-compose.yml**
```yaml
version: '3.8'

services:
  timescaledb:
    image: timescale/timescaledb:latest-pg15
    container_name: autonomous-timescaledb
    environment:
      POSTGRES_DB: autonomous_data
      POSTGRES_USER: autonomous
      POSTGRES_PASSWORD: ${DB_PASSWORD:-changeme}
      TIMESCALEDB_TELEMETRY: 'off'
    ports:
      - "5432:5432"
    volumes:
      - timescale_data:/var/lib/postgresql/data
      - ./init:/docker-entrypoint-initdb.d
    command: postgres -c shared_preload_libraries=timescaledb
    
  redis:
    image: redis:7-alpine
    container_name: autonomous-redis
    ports:
      - "6379:6379"
    volumes:
      - redis_data:/data
    command: redis-server --appendonly yes

  grafana:
    image: grafana/grafana:latest
    container_name: autonomous-grafana
    ports:
      - "3000:3000"
    environment:
      GF_SECURITY_ADMIN_PASSWORD: ${GRAFANA_PASSWORD:-admin}
    volumes:
      - grafana_data:/var/lib/grafana
      - ./grafana/dashboards:/etc/grafana/provisioning/dashboards
      - ./grafana/datasources:/etc/grafana/provisioning/datasources

volumes:
  timescale_data:
  redis_data:
  grafana_data:
```

### 2. Database Schema Template

**docker/data-platform/init/01-create-tables.sql**
```sql
-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- Create core data table (adapt for your domain)
CREATE TABLE IF NOT EXISTS time_series_data (
    id BIGSERIAL,
    timestamp TIMESTAMPTZ NOT NULL,
    source VARCHAR(50) NOT NULL,
    entity VARCHAR(100) NOT NULL,
    metric_name VARCHAR(50) NOT NULL,
    value DOUBLE PRECISION NOT NULL,
    metadata JSONB DEFAULT '{}',
    quality_score REAL DEFAULT 1.0,
    PRIMARY KEY (timestamp, source, entity, metric_name)
);

-- Convert to hypertable for time-series optimization
SELECT create_hypertable('time_series_data', 'timestamp', 
    chunk_time_interval => INTERVAL '1 hour',
    if_not_exists => TRUE
);

-- Create indexes for common queries
CREATE INDEX IF NOT EXISTS idx_time_series_source_entity 
    ON time_series_data (source, entity, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_time_series_metric 
    ON time_series_data (metric_name, timestamp DESC);

-- Create continuous aggregates for real-time analytics
CREATE MATERIALIZED VIEW IF NOT EXISTS hourly_metrics
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', timestamp) as hour,
    source,
    entity,
    metric_name,
    avg(value) as avg_value,
    min(value) as min_value,
    max(value) as max_value,
    count(*) as data_points
FROM time_series_data
GROUP BY hour, source, entity, metric_name
WITH NO DATA;

-- Add retention policy (adjust as needed)
SELECT add_retention_policy('time_series_data', INTERVAL '30 days');

-- Enable continuous aggregate refresh
SELECT add_continuous_aggregate_policy('hourly_metrics',
    start_offset => INTERVAL '1 hour',
    end_offset => INTERVAL '10 minutes',
    schedule_interval => INTERVAL '10 minutes'
);
```

## Neural Engine Integration

### 1. Core Neural Engine (src/neural/engine.rs)

```rust
use ruv_fann::{NetworkBuilder, ActivationFunction, TrainingAlgorithm};
use ruv_swarm_ml::{ModelFactory, ForecastingManager};
use anyhow::Result;
use std::collections::HashMap;
use tokio::sync::RwLock;
use std::sync::Arc;

pub struct NeuralEngine {
    models: Arc<RwLock<HashMap<String, Box<dyn NeuralModel>>>>,
    forecasting_manager: ForecastingManager,
    training_enabled: bool,
}

#[async_trait::async_trait]
pub trait NeuralModel: Send + Sync {
    async fn predict(&self, input: &[f64]) -> Result<Vec<f64>>;
    async fn update(&mut self, input: &[f64], target: &[f64]) -> Result<()>;
    fn model_type(&self) -> String;
    fn performance_metrics(&self) -> HashMap<String, f64>;
}

impl NeuralEngine {
    pub fn new() -> Result<Self> {
        let forecasting_manager = ForecastingManager::new(1024.0)?; // 1GB memory limit
        
        Ok(Self {
            models: Arc::new(RwLock::new(HashMap::new())),
            forecasting_manager,
            training_enabled: true,
        })
    }

    pub async fn register_model(&self, name: String, model: Box<dyn NeuralModel>) -> Result<()> {
        let mut models = self.models.write().await;
        models.insert(name, model);
        Ok(())
    }

    pub async fn predict(&self, model_name: &str, input: &[f64]) -> Result<Vec<f64>> {
        let models = self.models.read().await;
        match models.get(model_name) {
            Some(model) => model.predict(input).await,
            None => anyhow::bail!("Model {} not found", model_name),
        }
    }

    pub async fn batch_predict(&self, requests: Vec<(String, Vec<f64>)>) -> Result<Vec<Vec<f64>>> {
        let mut results = Vec::new();
        
        for (model_name, input) in requests {
            let prediction = self.predict(&model_name, &input).await?;
            results.push(prediction);
        }
        
        Ok(results)
    }
}

// Example implementation using ruv-FANN
pub struct FannModel {
    network: ruv_fann::Network<f32>,
    name: String,
    metrics: HashMap<String, f64>,
}

#[async_trait::async_trait]
impl NeuralModel for FannModel {
    async fn predict(&self, input: &[f64]) -> Result<Vec<f64>> {
        let input_f32: Vec<f32> = input.iter().map(|&x| x as f32).collect();
        let output = self.network.run(&input_f32)?;
        Ok(output.into_iter().map(|x| x as f64).collect())
    }

    async fn update(&mut self, input: &[f64], target: &[f64]) -> Result<()> {
        // Implementation depends on ruv-FANN training API
        // This will be available when ruv-FANN adds training support
        todo!("Training implementation pending ruv-FANN update")
    }

    fn model_type(&self) -> String {
        self.name.clone()
    }

    fn performance_metrics(&self) -> HashMap<String, f64> {
        self.metrics.clone()
    }
}
```

## Agent Layer (ruv-DAA Integration)

### 1. Base Agent Framework (src/agents/base_agent.rs)

```rust
use ruv_daa::{DaaOrchestrator, AgentCapability, Decision};
use ruv_swarm_core::{Agent, Task, CognitivePattern};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContext {
    pub timestamp: DateTime<Utc>,
    pub data: HashMap<String, serde_json::Value>,
    pub constraints: Vec<String>,
    pub objectives: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub confidence: f64,
    pub recommendation: String,
    pub reasoning: Vec<String>,
    pub metrics: HashMap<String, f64>,
    pub risk_assessment: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[async_trait]
pub trait AutonomousAgent: Send + Sync {
    async fn initialize(&mut self) -> anyhow::Result<()>;
    async fn analyze(&self, context: &AgentContext) -> anyhow::Result<AnalysisResult>;
    async fn decide(&self, analysis: &AnalysisResult) -> anyhow::Result<Decision>;
    async fn execute(&self, decision: &Decision) -> anyhow::Result<ExecutionResult>;
    async fn learn(&mut self, outcome: &ExecutionResult) -> anyhow::Result<()>;
    
    fn agent_id(&self) -> &str;
    fn capabilities(&self) -> Vec<AgentCapability>;
    fn status(&self) -> AgentStatus;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub value: f64,
    pub impact: HashMap<String, f64>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentStatus {
    Initializing,
    Active,
    Learning,
    Paused,
    Error(String),
}

// Template agent implementation
pub struct AnalyzerAgent {
    id: String,
    neural_model: String,
    cognitive_pattern: CognitivePattern,
    status: AgentStatus,
    learning_rate: f64,
}

impl AnalyzerAgent {
    pub fn new(id: String, neural_model: String) -> Self {
        Self {
            id,
            neural_model,
            cognitive_pattern: CognitivePattern::Analytical,
            status: AgentStatus::Initializing,
            learning_rate: 0.01,
        }
    }
}

#[async_trait]
impl AutonomousAgent for AnalyzerAgent {
    async fn initialize(&mut self) -> anyhow::Result<()> {
        // Initialize neural model connection
        // Setup data subscriptions
        self.status = AgentStatus::Active;
        Ok(())
    }

    async fn analyze(&self, context: &AgentContext) -> anyhow::Result<AnalysisResult> {
        // Extract features from context
        let features = self.extract_features(context)?;
        
        // Get neural network prediction
        let prediction = self.neural_predict(&features).await?;
        
        // Apply domain-specific logic
        let analysis = self.interpret_prediction(prediction, context)?;
        
        Ok(analysis)
    }

    async fn decide(&self, analysis: &AnalysisResult) -> anyhow::Result<Decision> {
        // Use ruv-DAA decision framework
        let decision = Decision::builder()
            .confidence(analysis.confidence)
            .action(analysis.recommendation.clone())
            .reasoning(analysis.reasoning.clone())
            .build();
            
        Ok(decision)
    }

    async fn execute(&self, decision: &Decision) -> anyhow::Result<ExecutionResult> {
        // Execute the decision
        // This is domain-specific implementation
        todo!("Implement domain-specific execution logic")
    }

    async fn learn(&mut self, outcome: &ExecutionResult) -> anyhow::Result<()> {
        // Update neural model based on outcome
        if outcome.success {
            self.learning_rate *= 1.01; // Increase confidence
        } else {
            self.learning_rate *= 0.99; // Decrease learning rate
        }
        Ok(())
    }

    fn agent_id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::DataAnalysis,
            AgentCapability::PatternRecognition,
            AgentCapability::RiskAssessment,
        ]
    }

    fn status(&self) -> AgentStatus {
        self.status.clone()
    }
}

impl AnalyzerAgent {
    async fn neural_predict(&self, features: &[f64]) -> anyhow::Result<Vec<f64>> {
        // Call neural engine
        todo!("Integrate with neural engine")
    }
    
    fn extract_features(&self, context: &AgentContext) -> anyhow::Result<Vec<f64>> {
        // Extract numerical features from context data
        todo!("Implement feature extraction")
    }
    
    fn interpret_prediction(&self, prediction: Vec<f64>, context: &AgentContext) -> anyhow::Result<AnalysisResult> {
        // Interpret neural network output in domain context
        todo!("Implement prediction interpretation")
    }
}
```

## Data Connector Framework

### 1. Connector Template (connectors/Dockerfile.template)

```dockerfile
FROM rust:1.75-slim as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/connector /usr/local/bin/connector

ENV RUST_LOG=info
EXPOSE 8080

CMD ["connector"]
```

### 2. Base Connector Framework (connectors/common/src/lib.rs)

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::time::{Duration, interval};
use anyhow::Result;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub timestamp: DateTime<Utc>,
    pub source: String,
    pub entity: String,
    pub metric: String,
    pub value: f64,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorConfig {
    pub name: String,
    pub endpoint: String,
    pub api_key: Option<String>,
    pub rate_limit_per_minute: u32,
    pub batch_size: usize,
    pub retry_attempts: u32,
    pub health_check_interval: Duration,
}

#[async_trait]
pub trait DataConnector: Send + Sync {
    async fn initialize(&mut self, config: ConnectorConfig) -> Result<()>;
    async fn fetch_data(&self) -> Result<Vec<DataPoint>>;
    async fn health_check(&self) -> Result<bool>;
    async fn shutdown(&self) -> Result<()>;
    
    fn connector_name(&self) -> &str;
    fn supported_entities(&self) -> Vec<String>;
}

pub struct ConnectorService {
    connector: Box<dyn DataConnector>,
    config: ConnectorConfig,
    is_running: bool,
}

impl ConnectorService {
    pub fn new(connector: Box<dyn DataConnector>, config: ConnectorConfig) -> Self {
        Self {
            connector,
            config,
            is_running: false,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        self.connector.initialize(self.config.clone()).await?;
        self.is_running = true;

        let mut fetch_interval = interval(Duration::from_secs(60)); // 1 minute default
        let mut health_interval = interval(self.config.health_check_interval);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = fetch_interval.tick() => {
                        // Fetch and send data
                        if let Ok(data) = self.connector.fetch_data().await {
                            self.send_to_platform(data).await;
                        }
                    }
                    _ = health_interval.tick() => {
                        // Health check
                        if self.connector.health_check().await.is_err() {
                            tracing::warn!("Health check failed for {}", self.config.name);
                        }
                    }
                }
            }
        });

        Ok(())
    }

    async fn send_to_platform(&self, data: Vec<DataPoint>) {
        // Send data to the main platform via HTTP/gRPC/message queue
        // Implementation depends on your platform's ingestion API
        todo!("Implement data transmission to platform")
    }
}

// Example REST API connector
pub struct RestApiConnector {
    name: String,
    client: reqwest::Client,
    endpoint: String,
    api_key: Option<String>,
}

#[async_trait]
impl DataConnector for RestApiConnector {
    async fn initialize(&mut self, config: ConnectorConfig) -> Result<()> {
        self.name = config.name;
        self.endpoint = config.endpoint;
        self.api_key = config.api_key;
        self.client = reqwest::Client::new();
        Ok(())
    }

    async fn fetch_data(&self) -> Result<Vec<DataPoint>> {
        let mut request = self.client.get(&self.endpoint);
        
        if let Some(api_key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = request.send().await?;
        let raw_data: serde_json::Value = response.json().await?;
        
        // Parse response into DataPoint format
        let data_points = self.parse_response(raw_data)?;
        
        Ok(data_points)
    }

    async fn health_check(&self) -> Result<bool> {
        let response = self.client.get(&format!("{}/health", self.endpoint)).send().await?;
        Ok(response.status().is_success())
    }

    async fn shutdown(&self) -> Result<()> {
        // Cleanup resources
        Ok(())
    }

    fn connector_name(&self) -> &str {
        &self.name
    }

    fn supported_entities(&self) -> Vec<String> {
        // Return list of entities this connector can provide
        vec!["entity1".to_string(), "entity2".to_string()]
    }
}

impl RestApiConnector {
    fn parse_response(&self, data: serde_json::Value) -> Result<Vec<DataPoint>> {
        // Parse API response into standardized DataPoint format
        // This is connector-specific logic
        todo!("Implement response parsing")
    }
}
```

## MCP Integration

### 1. Platform MCP Server (src/mcp/server.rs)

```rust
use axum::{
    extract::ws::{WebSocket, Message},
    response::Response,
    routing::get,
    Router,
};
use ruv_swarm_mcp::McpServer;
use serde_json::Value;
use anyhow::Result;

pub struct PlatformMcpServer {
    neural_engine: Arc<crate::neural::NeuralEngine>,
    agent_registry: Arc<crate::agents::AgentRegistry>,
}

impl PlatformMcpServer {
    pub fn new(
        neural_engine: Arc<crate::neural::NeuralEngine>,
        agent_registry: Arc<crate::agents::AgentRegistry>,
    ) -> Self {
        Self {
            neural_engine,
            agent_registry,
        }
    }

    pub async fn start(&self, port: u16) -> Result<()> {
        let app = Router::new()
            .route("/mcp", get(Self::mcp_handler))
            .with_state(self.clone());

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
        axum::serve(listener, app).await?;
        
        Ok(())
    }

    async fn mcp_handler(ws: WebSocket) {
        // Handle MCP WebSocket connections
        // Integrate with ruv-swarm-mcp
        todo!("Implement MCP message handling")
    }
}

// Platform-specific MCP tools
pub struct PlatformTools;

impl PlatformTools {
    pub async fn predict(&self, model_name: String, input: Vec<f64>) -> Result<Value> {
        // Neural prediction tool
        todo!("Implement prediction tool")
    }

    pub async fn create_agent(&self, agent_type: String, config: Value) -> Result<Value> {
        // Agent creation tool
        todo!("Implement agent creation tool")
    }

    pub async fn get_data(&self, query: String) -> Result<Value> {
        // Data query tool
        todo!("Implement data query tool")
    }

    pub async fn get_metrics(&self) -> Result<Value> {
        // Platform metrics tool
        todo!("Implement metrics tool")
    }
}
```

## Quick Start Script

### 1. Platform Initialization (quick-start.sh)

```bash
#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 Autonomous Neural Platform Quick Start${NC}"
echo "=============================================="

# Check dependencies
check_dependencies() {
    echo -e "${YELLOW}Checking dependencies...${NC}"
    
    if ! command -v docker &> /dev/null; then
        echo -e "${RED}❌ Docker is required but not installed${NC}"
        exit 1
    fi
    
    if ! command -v docker-compose &> /dev/null; then
        echo -e "${RED}❌ Docker Compose is required but not installed${NC}"
        exit 1
    fi
    
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}❌ Rust/Cargo is required but not installed${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}✅ All dependencies found${NC}"
}

# Setup environment
setup_environment() {
    echo -e "${YELLOW}Setting up environment...${NC}"
    
    # Create environment file if it doesn't exist
    if [ ! -f .env ]; then
        cp .env.example .env
        echo -e "${GREEN}✅ Created .env file from template${NC}"
        echo -e "${YELLOW}⚠️  Please edit .env with your configuration${NC}"
    fi
    
    # Create data directories
    mkdir -p data/{timescale,redis,grafana}
    mkdir -p logs
    
    echo -e "${GREEN}✅ Environment setup complete${NC}"
}

# Start data platform
start_data_platform() {
    echo -e "${YELLOW}Starting data platform...${NC}"
    
    cd docker/data-platform
    docker-compose up -d
    cd ../..
    
    # Wait for databases to be ready
    echo -e "${YELLOW}Waiting for databases to initialize...${NC}"
    sleep 10
    
    # Run database migrations
    echo -e "${YELLOW}Running database migrations...${NC}"
    cargo run --bin migrate
    
    echo -e "${GREEN}✅ Data platform started${NC}"
}

# Build and start platform
start_platform() {
    echo -e "${YELLOW}Building platform...${NC}"
    cargo build --release
    
    echo -e "${YELLOW}Starting neural engine...${NC}"
    cargo run --release --bin platform &
    PLATFORM_PID=$!
    
    # Start MCP server
    echo -e "${YELLOW}Starting MCP server...${NC}"
    cargo run --release --bin mcp-server &
    MCP_PID=$!
    
    echo -e "${GREEN}✅ Platform started${NC}"
    echo -e "${BLUE}Platform PID: $PLATFORM_PID${NC}"
    echo -e "${BLUE}MCP Server PID: $MCP_PID${NC}"
    
    # Save PIDs for cleanup
    echo $PLATFORM_PID > platform.pid
    echo $MCP_PID > mcp.pid
}

# Stop platform
stop_platform() {
    echo -e "${YELLOW}Stopping platform...${NC}"
    
    if [ -f platform.pid ]; then
        kill $(cat platform.pid) 2>/dev/null || true
        rm platform.pid
    fi
    
    if [ -f mcp.pid ]; then
        kill $(cat mcp.pid) 2>/dev/null || true
        rm mcp.pid
    fi
    
    cd docker/data-platform
    docker-compose down
    cd ../..
    
    echo -e "${GREEN}✅ Platform stopped${NC}"
}

# Show status
show_status() {
    echo -e "${BLUE}Platform Status${NC}"
    echo "==============="
    
    # Check data platform
    cd docker/data-platform
    if docker-compose ps | grep -q "Up"; then
        echo -e "${GREEN}✅ Data Platform: Running${NC}"
    else
        echo -e "${RED}❌ Data Platform: Stopped${NC}"
    fi
    cd ../..
    
    # Check main platform
    if [ -f platform.pid ] && kill -0 $(cat platform.pid) 2>/dev/null; then
        echo -e "${GREEN}✅ Neural Platform: Running (PID: $(cat platform.pid))${NC}"
    else
        echo -e "${RED}❌ Neural Platform: Stopped${NC}"
    fi
    
    # Check MCP server
    if [ -f mcp.pid ] && kill -0 $(cat mcp.pid) 2>/dev/null; then
        echo -e "${GREEN}✅ MCP Server: Running (PID: $(cat mcp.pid))${NC}"
    else
        echo -e "${RED}❌ MCP Server: Stopped${NC}"
    fi
    
    echo ""
    echo -e "${BLUE}Service URLs:${NC}"
    echo "  Grafana: http://localhost:3000"
    echo "  MCP Server: ws://localhost:8080/mcp"
    echo "  TimescaleDB: postgresql://localhost:5432/autonomous_data"
    echo "  Redis: redis://localhost:6379"
}

# Main menu
show_menu() {
    echo ""
    echo -e "${PURPLE}Available Commands:${NC}"
    echo "  1) setup     - Initial environment setup"
    echo "  2) start     - Start the complete platform"
    echo "  3) stop      - Stop the platform"
    echo "  4) restart   - Restart the platform"
    echo "  5) status    - Show platform status"
    echo "  6) logs      - Show platform logs"
    echo "  7) clean     - Clean all data and stop"
    echo "  8) help      - Show this menu"
    echo ""
}

# Handle command line arguments
case "${1:-help}" in
    setup)
        check_dependencies
        setup_environment
        ;;
    start)
        check_dependencies
        setup_environment
        start_data_platform
        start_platform
        show_status
        ;;
    stop)
        stop_platform
        ;;
    restart)
        stop_platform
        sleep 2
        start_data_platform
        start_platform
        ;;
    status)
        show_status
        ;;
    logs)
        tail -f logs/*.log 2>/dev/null || echo "No logs found"
        ;;
    clean)
        stop_platform
        docker system prune -f
        rm -rf data/* logs/*
        echo -e "${GREEN}✅ Cleanup complete${NC}"
        ;;
    help|*)
        show_menu
        ;;
esac
```

## Configuration Template

### 1. Platform Configuration (config/platform.toml)

```toml
[platform]
name = "autonomous-platform"
version = "0.1.0"
environment = "development"

[neural]
memory_limit_mb = 1024
model_cache_size = 100
training_enabled = true
gpu_enabled = false

[agents]
max_concurrent = 10
health_check_interval = 30
auto_restart = true

[data]
retention_days = 30
batch_size = 1000
compression_enabled = true

[database]
host = "localhost"
port = 5432
database = "autonomous_data"
username = "autonomous"
password = "${DB_PASSWORD}"
max_connections = 20
connection_timeout = 30

[redis]
host = "localhost"
port = 6379
database = 0
max_connections = 10

[mcp]
host = "0.0.0.0"
port = 8080
max_connections = 100
auth_required = false

[monitoring]
metrics_enabled = true
tracing_enabled = true
log_level = "info"
```

## Usage Examples

### 1. Basic Usage Example (examples/basic_usage.rs)

```rust
use autonomous_platform::{Platform, neural::NeuralEngine, agents::AgentRegistry};
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize platform
    let mut platform = Platform::new().await?;
    
    // Create neural engine
    let neural_engine = NeuralEngine::new()?;
    
    // Create and register an analyzer agent
    let agent = AnalyzerAgent::new("analyzer_1".to_string(), "lstm_model".to_string());
    platform.register_agent(Box::new(agent)).await?;
    
    // Start platform
    platform.start().await?;
    
    // Example: Analyze some data
    let context = AgentContext {
        timestamp: chrono::Utc::now(),
        data: std::collections::HashMap::new(),
        constraints: vec![],
        objectives: vec!["maximize_efficiency".to_string()],
    };
    
    let analysis = platform.analyze("analyzer_1", &context).await?;
    println!("Analysis result: {:?}", analysis);
    
    Ok(())
}
```

This comprehensive setup guide provides a foundation for building autonomous neural decision-making platforms that can be adapted for trading, IoT, recommendations, or any domain requiring intelligent automation.