# DAA Docker Container Integration Plan

## Overview
Instead of trying to integrate an unknown library API, we'll run DAA as a Docker container and connect our neural-trader platform to it via standard protocols (HTTP, WebSocket, gRPC).

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   Neural Trader Platform                      │
│  ┌────────────┐  ┌──────────────┐  ┌──────────────────┐    │
│  │   FANN     │  │  Trading     │  │   Platform       │    │
│  │  Models    │  │  Logic       │  │  Orchestrator    │    │
│  └────────────┘  └──────────────┘  └──────────────────┘    │
└─────────────────────────┬────────────────────────────────────┘
                          │ API/WebSocket/gRPC
┌─────────────────────────┴────────────────────────────────────┐
│                    DAA Container Ecosystem                    │
│  ┌────────────┐  ┌──────────────┐  ┌──────────────────┐    │
│  │    DAA     │  │   MCP        │  │   Event Bus     │    │
│  │   Agent    │  │  Server      │  │   & Pipeline    │    │
│  └────────────┘  └──────────────┘  └──────────────────┘    │
└─────────────────────────┬────────────────────────────────────┘
                          │ Network
┌─────────────────────────┴────────────────────────────────────┐
│                    Shared Data Platform                       │
│  ┌────────────┐  ┌──────────────┐  ┌──────────────────┐    │
│  │TimescaleDB │  │    Redis     │  │   Prometheus    │    │
│  │            │  │              │  │   + Grafana     │    │
│  └────────────┘  └──────────────┘  └──────────────────┘    │
└──────────────────────────────────────────────────────────────┘
```

## Phase 1: Docker Environment Setup (Day 1)

### 1.1 Create Docker Compose Configuration
```yaml
# docker-compose.daa.yml
version: '3.8'

services:
  # DAA Agent Container
  daa-agent:
    image: ghcr.io/ruvnet/daa:latest  # or build from source
    container_name: neural-trader-daa
    environment:
      - DAA_AGENT_ID=neural-trader-01
      - DAA_MODE=trading
      - DAA_NETWORK_MODE=standalone
      - DATABASE_URL=postgresql://postgres:password@timescaledb:5432/trading
      - REDIS_URL=redis://redis:6379
      - MCP_SERVER_ENABLED=true
      - MCP_SERVER_PORT=3333
      - EVENT_BUS_PORT=4444
      - API_PORT=8080
      - ENABLE_METRICS=true
    volumes:
      - ./config/daa:/config
      - ./data/daa:/data
    ports:
      - "8080:8080"   # REST API
      - "3333:3333"   # MCP Server
      - "4444:4444"   # Event Bus WebSocket
      - "9090:9090"   # Metrics
    networks:
      - neural-net
    depends_on:
      - timescaledb
      - redis

  # Our existing TimescaleDB
  timescaledb:
    image: timescale/timescaledb:latest-pg14
    container_name: neural-trader-db
    environment:
      - POSTGRES_DB=trading
      - POSTGRES_USER=postgres
      - POSTGRES_PASSWORD=password
    volumes:
      - timescale_data:/var/lib/postgresql/data
    ports:
      - "5432:5432"
    networks:
      - neural-net

  # Our existing Redis
  redis:
    image: redis:7-alpine
    container_name: neural-trader-cache
    ports:
      - "6379:6379"
    networks:
      - neural-net
    volumes:
      - redis_data:/data

  # MCP Tools Registry (optional)
  mcp-registry:
    image: ghcr.io/ruvnet/mcp-registry:latest
    container_name: mcp-registry
    environment:
      - REGISTRY_PORT=5555
    ports:
      - "5555:5555"
    networks:
      - neural-net

  # Monitoring Stack
  prometheus:
    image: prom/prometheus:latest
    container_name: neural-trader-prometheus
    volumes:
      - ./config/prometheus:/etc/prometheus
      - prometheus_data:/prometheus
    ports:
      - "9091:9090"
    networks:
      - neural-net

  grafana:
    image: grafana/grafana:latest
    container_name: neural-trader-grafana
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    volumes:
      - grafana_data:/var/lib/grafana
      - ./config/grafana:/etc/grafana/provisioning
    ports:
      - "3000:3000"
    networks:
      - neural-net

networks:
  neural-net:
    driver: bridge

volumes:
  timescale_data:
  redis_data:
  prometheus_data:
  grafana_data:
```

### 1.2 DAA Configuration File
```toml
# config/daa/daa.toml
[agent]
id = "neural-trader-01"
name = "Neural Trading Agent"
description = "Autonomous trading agent with FANN neural predictions"

[network]
mode = "standalone"  # or "cluster" for multi-agent
peers = []

[orchestration]
max_concurrent_decisions = 10
decision_timeout_ms = 5000
enable_consensus = false  # true for multi-agent

[ai_integration]
provider = "openai"  # or "anthropic" 
model = "gpt-4"
mcp_enabled = true
cache_predictions = true

[data_pipeline]
input_sources = ["timescaledb", "redis", "websocket"]
processors = ["normalize", "aggregate", "predict"]
output_sinks = ["decisions", "metrics", "storage"]
batch_size = 100
buffer_size = 10000

[rules_engine]
enabled = true
rule_sets = ["trading_rules", "risk_rules"]
evaluation_mode = "sequential"

[economy]
enabled = false  # Enable later for tokenomics
token_address = ""
```

### 1.3 Neural Trader API Configuration
```rust
// src/config/daa_client.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DaaClientConfig {
    pub api_url: String,
    pub mcp_url: String,
    pub event_bus_url: String,
    pub auth_token: Option<String>,
    pub timeout_secs: u64,
}

impl Default for DaaClientConfig {
    fn default() -> Self {
        Self {
            api_url: "http://localhost:8080".to_string(),
            mcp_url: "ws://localhost:3333".to_string(),
            event_bus_url: "ws://localhost:4444".to_string(),
            auth_token: None,
            timeout_secs: 30,
        }
    }
}
```

## Phase 2: API Client Implementation (Day 2)

### 2.1 DAA HTTP/WebSocket Client
```rust
// src/clients/daa_client.rs
use reqwest::Client;
use tokio_tungstenite::{connect_async, WebSocketStream};
use futures_util::{StreamExt, SinkExt};

pub struct DaaClient {
    http_client: Client,
    config: DaaClientConfig,
    event_bus: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
}

impl DaaClient {
    pub async fn new(config: DaaClientConfig) -> Result<Self> {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()?;
            
        let mut client = Self {
            http_client,
            config,
            event_bus: None,
        };
        
        // Connect to event bus
        client.connect_event_bus().await?;
        
        Ok(client)
    }
    
    /// Register our neural trader as an agent
    pub async fn register_agent(&self, agent_config: AgentConfig) -> Result<String> {
        let response = self.http_client
            .post(&format!("{}/api/v1/agents", self.config.api_url))
            .json(&agent_config)
            .send()
            .await?;
            
        let result: AgentRegistration = response.json().await?;
        Ok(result.agent_id)
    }
    
    /// Submit a decision request to DAA
    pub async fn request_decision(&self, context: DecisionContext) -> Result<Decision> {
        let response = self.http_client
            .post(&format!("{}/api/v1/decisions", self.config.api_url))
            .json(&context)
            .send()
            .await?;
            
        let decision: Decision = response.json().await?;
        Ok(decision)
    }
    
    /// Connect to DAA event bus
    async fn connect_event_bus(&mut self) -> Result<()> {
        let (ws_stream, _) = connect_async(&self.config.event_bus_url).await?;
        self.event_bus = Some(ws_stream);
        
        // Start event listener
        self.start_event_listener().await;
        
        Ok(())
    }
    
    /// Listen for DAA events
    async fn start_event_listener(&self) {
        if let Some(mut stream) = self.event_bus {
            tokio::spawn(async move {
                while let Some(msg) = stream.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Ok(event) = serde_json::from_str::<DaaEvent>(&text) {
                                // Handle event
                                handle_daa_event(event).await;
                            }
                        }
                        _ => {}
                    }
                }
            });
        }
    }
    
    /// Send prediction result to DAA
    pub async fn send_prediction(&self, prediction: PredictionResult) -> Result<()> {
        self.http_client
            .post(&format!("{}/api/v1/predictions", self.config.api_url))
            .json(&prediction)
            .send()
            .await?;
            
        Ok(())
    }
}
```

### 2.2 MCP Client for Tool Integration
```rust
// src/clients/mcp_client.rs
pub struct McpClient {
    ws_client: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl McpClient {
    pub async fn connect(url: &str) -> Result<Self> {
        let (ws_stream, _) = connect_async(url).await?;
        Ok(Self { ws_client: ws_stream })
    }
    
    /// Register custom trading tools
    pub async fn register_tools(&mut self) -> Result<()> {
        let tools = vec![
            Tool {
                name: "get_market_data".to_string(),
                description: "Retrieve market data from TimescaleDB".to_string(),
                parameters: json!({
                    "symbol": "string",
                    "timeframe": "string",
                    "limit": "integer"
                }),
            },
            Tool {
                name: "execute_trade".to_string(),
                description: "Execute a trading decision".to_string(),
                parameters: json!({
                    "action": "buy|sell",
                    "symbol": "string",
                    "amount": "number"
                }),
            },
            Tool {
                name: "get_neural_prediction".to_string(),
                description: "Get FANN neural network prediction".to_string(),
                parameters: json!({
                    "symbol": "string",
                    "model": "string",
                    "horizon": "integer"
                }),
            },
        ];
        
        for tool in tools {
            self.ws_client.send(Message::Text(
                serde_json::to_string(&McpMessage::RegisterTool(tool))?
            )).await?;
        }
        
        Ok(())
    }
}
```

## Phase 3: Integration Layer (Day 3)

### 3.1 Bridge Between Neural Trader and DAA
```rust
// src/integration/daa_bridge.rs
use crate::clients::{DaaClient, McpClient};
use crate::integration::NeuralPredictionSystem;

pub struct DaaBridge {
    daa_client: Arc<DaaClient>,
    mcp_client: Arc<Mutex<McpClient>>,
    neural_system: Arc<NeuralPredictionSystem>,
    event_handler: Arc<DaaEventHandler>,
}

impl DaaBridge {
    pub async fn new(config: DaaClientConfig) -> Result<Self> {
        // Initialize clients
        let daa_client = Arc::new(DaaClient::new(config.clone()).await?);
        let mcp_client = Arc::new(Mutex::new(
            McpClient::connect(&config.mcp_url).await?
        ));
        
        // Register as trading agent
        let agent_config = AgentConfig {
            name: "neural-trader".to_string(),
            capabilities: vec!["predict", "trade", "analyze"],
            resources: ResourceRequirements {
                cpu: 4,
                memory_gb: 8,
                gpu: true,
            },
        };
        
        let agent_id = daa_client.register_agent(agent_config).await?;
        info!("Registered with DAA as agent: {}", agent_id);
        
        // Register MCP tools
        mcp_client.lock().await.register_tools().await?;
        
        Ok(Self {
            daa_client,
            mcp_client,
            neural_system: Arc::new(NeuralPredictionSystem::new(8.0).await?),
            event_handler: Arc::new(DaaEventHandler::new()),
        })
    }
    
    /// Handle incoming DAA events
    pub async fn handle_event(&self, event: DaaEvent) -> Result<()> {
        match event {
            DaaEvent::PredictionRequested { context, callback_id } => {
                // Get prediction from our neural system
                let prediction = self.neural_system
                    .get_prediction_for_decision(context.into())
                    .await?;
                
                // Send back to DAA
                self.daa_client.send_prediction(prediction).await?;
            }
            
            DaaEvent::MarketDataReceived { data } => {
                // Store in our TimescaleDB
                self.store_market_data(data).await?;
            }
            
            DaaEvent::DecisionRequired { context } => {
                // Make decision using our logic + DAA rules
                let decision = self.make_trading_decision(context).await?;
                self.daa_client.submit_decision(decision).await?;
            }
            
            _ => {
                self.event_handler.handle(event).await?;
            }
        }
        
        Ok(())
    }
}
```

### 3.2 Modified Platform Orchestrator
```rust
// src/integration/platform_orchestrator.rs
impl PlatformOrchestrator {
    pub async fn new(config: PlatformConfig) -> Result<Self> {
        // ... existing initialization ...
        
        // Add DAA bridge
        let daa_bridge = if config.daa_enabled {
            Some(DaaBridge::new(config.daa_client.clone()).await?)
        } else {
            None
        };
        
        Ok(Self {
            streaming_pipeline,
            neural_system,
            daa_bridge,  // NEW
            // ... rest of fields
        })
    }
    
    pub async fn run(&self) -> Result<()> {
        info!("Starting Platform Orchestrator with DAA integration");
        
        // Start DAA event processing if enabled
        if let Some(bridge) = &self.daa_bridge {
            tokio::spawn({
                let bridge = bridge.clone();
                async move {
                    // This will be handled by DAA's event bus
                    loop {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            });
        }
        
        // Rest of existing run logic...
    }
}
```

## Phase 4: Data Pipeline Integration (Day 4)

### 4.1 Connect Our Pipeline to DAA's
```rust
// src/integration/pipeline_connector.rs
pub struct PipelineConnector {
    daa_client: Arc<DaaClient>,
    local_pipeline: Arc<DataPipeline>,
}

impl PipelineConnector {
    /// Stream data to DAA's pipeline
    pub async fn stream_to_daa(&self, data: TimeSeriesData) -> Result<()> {
        let daa_format = DaaMarketData {
            timestamp: data.timestamp,
            symbol: data.symbol,
            values: data.values,
            metadata: json!({
                "source": "neural-trader",
                "quality": data.quality_metrics
            }),
        };
        
        self.daa_client.send_to_pipeline(daa_format).await?;
        Ok(())
    }
    
    /// Subscribe to DAA pipeline outputs
    pub async fn subscribe_to_daa_pipeline(&self) -> Result<()> {
        self.daa_client.subscribe_pipeline("processed_data", |data| {
            // Handle processed data from DAA
            self.handle_daa_data(data).await
        }).await?;
        
        Ok(())
    }
}
```

## Phase 5: Production Deployment (Day 5)

### 5.1 Production Docker Compose
```yaml
# docker-compose.prod.yml
version: '3.8'

services:
  # Load balancer for DAA cluster
  daa-lb:
    image: nginx:alpine
    volumes:
      - ./config/nginx/daa-lb.conf:/etc/nginx/nginx.conf
    ports:
      - "80:80"
      - "443:443"
    networks:
      - neural-net

  # Multiple DAA agents for HA
  daa-agent-1:
    extends:
      file: docker-compose.daa.yml
      service: daa-agent
    container_name: daa-agent-1
    environment:
      - DAA_AGENT_ID=neural-trader-01
      - DAA_CLUSTER_SEED=daa-agent-1

  daa-agent-2:
    extends:
      file: docker-compose.daa.yml
      service: daa-agent
    container_name: daa-agent-2
    environment:
      - DAA_AGENT_ID=neural-trader-02
      - DAA_CLUSTER_PEERS=daa-agent-1

  daa-agent-3:
    extends:
      file: docker-compose.daa.yml
      service: daa-agent
    container_name: daa-agent-3
    environment:
      - DAA_AGENT_ID=neural-trader-03
      - DAA_CLUSTER_PEERS=daa-agent-1,daa-agent-2

  # Shared data platform with replication
  timescaledb-primary:
    extends:
      file: docker-compose.daa.yml
      service: timescaledb
    environment:
      - POSTGRESQL_REPLICATION_MODE=master
      - POSTGRESQL_REPLICATION_USER=replicator
      - POSTGRESQL_REPLICATION_PASSWORD=repl_password

  timescaledb-replica:
    image: timescale/timescaledb:latest-pg14
    environment:
      - POSTGRESQL_REPLICATION_MODE=slave
      - POSTGRESQL_MASTER_HOST=timescaledb-primary
      - POSTGRESQL_REPLICATION_USER=replicator
      - POSTGRESQL_REPLICATION_PASSWORD=repl_password
```

### 5.2 Monitoring Configuration
```yaml
# config/prometheus/prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'daa-agents'
    static_configs:
      - targets: 
        - 'daa-agent-1:9090'
        - 'daa-agent-2:9090'
        - 'daa-agent-3:9090'
        
  - job_name: 'neural-trader'
    static_configs:
      - targets: ['neural-trader:9091']
        
  - job_name: 'timescaledb'
    static_configs:
      - targets: ['timescaledb-primary:9187']
```

## Phase 6: Testing & Migration (Day 6-7)

### 6.1 Integration Tests
```rust
#[tokio::test]
async fn test_daa_container_integration() {
    // Start test containers
    let docker = Docker::connect_with_local_defaults().unwrap();
    
    // Start DAA container
    let daa_container = docker.run(
        "ghcr.io/ruvnet/daa:test",
        vec!["-e", "DAA_MODE=test"]
    ).await.unwrap();
    
    // Connect our platform
    let config = DaaClientConfig {
        api_url: format!("http://localhost:{}", daa_container.port),
        ..Default::default()
    };
    
    let bridge = DaaBridge::new(config).await.unwrap();
    
    // Test registration
    assert!(bridge.is_registered());
    
    // Test prediction flow
    let context = create_test_context();
    let prediction = bridge.request_prediction(context).await.unwrap();
    assert!(prediction.confidence > 0.0);
}
```

## Key Benefits of Docker Approach

1. **No Library Mystery**: We don't need to figure out the Rust API
2. **Standard Protocols**: HTTP/WebSocket/gRPC are well-documented
3. **Independent Scaling**: Scale DAA and Neural Trader separately
4. **Easy Updates**: Just pull new DAA images
5. **Production Ready**: Docker Compose handles networking, volumes, etc.
6. **Shared Data Platform**: TimescaleDB and Redis work for both systems
7. **Built-in Features**: MCP server, event bus, monitoring all included

## Migration Checklist

- [ ] Create Docker Compose files
- [ ] Implement DAA HTTP/WebSocket clients
- [ ] Create bridge between systems
- [ ] Connect data pipelines
- [ ] Set up monitoring
- [ ] Test with containers
- [ ] Deploy to production

## Success Metrics

- DAA container running and healthy
- Neural Trader registered as DAA agent
- Events flowing between systems
- Predictions processed through DAA
- Data pipeline connected
- MCP tools accessible
- Monitoring showing both systems