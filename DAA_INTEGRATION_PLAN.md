# DAA Integration Implementation Plan

## Overview
This plan migrates our custom implementation to use the ACTUAL daa-orchestrator library components instead of reimplementing them.

## Phase 1: Discovery & Setup (Day 1)

### 1.1 Explore DAA Library API
```rust
// Import ACTUAL DAA components
use daa_orchestrator::{
    DaaOrchestrator,      // The REAL orchestrator
    Agent,                // The REAL agent type
    Decision,             // The REAL decision type
    EventBus,             // Built-in event system
    DataPipeline,         // Built-in data pipeline
    RulesEngine,          // Dynamic rules system
    EconomyManager,       // Resource management
};
```

### 1.2 Set Up DAA Deployment
```yaml
# docker-compose.yml for DAA
version: '3.8'
services:
  daa-agent:
    image: daa-orchestrator:latest
    environment:
      - DAA_MODE=trading
      - DAA_CONFIG=/config/daa.toml
    volumes:
      - ./config:/config
    
  # Our existing services integrate with DAA
  timescaledb:
    # Existing config
  
  redis:
    # Existing config
```

### 1.3 Initialize Real DAA Components
```rust
// src/integration/real_daa_integration.rs
use daa_orchestrator::{DaaOrchestrator, Config as DaaConfig};

pub async fn initialize_real_daa(config: &PlatformConfig) -> Result<DaaOrchestrator> {
    // Use the ACTUAL DAA initialization
    let daa_config = DaaConfig {
        agent_id: "neural-trader-01",
        network_mode: "standalone",
        data_pipeline: DataPipelineConfig {
            input_sources: vec!["timescaledb", "redis"],
            output_sinks: vec!["predictions", "decisions"],
        },
        ai_integration: AiConfig {
            model: "gpt-4", // or switch to Claude as DAA expects
            mcp_enabled: true,
        },
    };
    
    DaaOrchestrator::new(daa_config).await?
}
```

## Phase 2: Data Pipeline Integration (Day 2)

### 2.1 Replace Our Pipeline with DAA's
```rust
// REMOVE: src/data/pipeline.rs (our custom implementation)
// USE: DAA's built-in DataPipeline

use daa_orchestrator::pipeline::{DataPipeline, StreamProcessor};

pub struct TradingDataProcessor {
    daa_pipeline: DataPipeline,
}

impl TradingDataProcessor {
    pub async fn new(daa: &DaaOrchestrator) -> Result<Self> {
        // Use DAA's pipeline, not ours
        let pipeline = daa.create_pipeline("trading_pipeline")
            .with_source("market_data")
            .with_processor(Box::new(TimeSeriesProcessor))
            .with_sink("neural_predictions")
            .build()?;
        
        Ok(Self { daa_pipeline: pipeline })
    }
}
```

### 2.2 Connect TimescaleDB to DAA Pipeline
```rust
// Create DAA-compatible data source
impl DataSource for TimescaleDBSource {
    async fn poll(&mut self) -> Result<Vec<DaaEvent>> {
        let data = self.storage.get_latest_data().await?;
        Ok(data.into_iter()
            .map(|ts| DaaEvent::MarketData(ts.into()))
            .collect())
    }
}
```

## Phase 3: Event-Driven Architecture (Day 3)

### 3.1 Implement DAA Event Handlers
```rust
use daa_orchestrator::{EventHandler, DaaEvent};

pub struct NeuralTradingAgent {
    neural_system: Arc<NeuralPredictionSystem>,
}

#[async_trait]
impl EventHandler for NeuralTradingAgent {
    async fn handle_event(&self, event: DaaEvent) -> Result<()> {
        match event {
            DaaEvent::MarketDataReceived(data) => {
                // Process with neural network
                let prediction = self.neural_system.predict(data).await?;
                self.emit(DaaEvent::PredictionGenerated(prediction)).await?;
            }
            DaaEvent::DecisionRequested(context) => {
                // Make trading decision
                let decision = self.make_decision(context).await?;
                self.emit(DaaEvent::DecisionMade(decision)).await?;
            }
            _ => {}
        }
        Ok(())
    }
}
```

### 3.2 Register Agents with Real DAA
```rust
pub async fn setup_trading_agents(daa: &mut DaaOrchestrator) -> Result<()> {
    // Register our agents with the REAL DAA orchestrator
    daa.register_agent(
        Agent::new("neural_trader")
            .with_handler(Box::new(NeuralTradingAgent::new()))
            .with_capabilities(vec!["predict", "trade", "analyze"])
    ).await?;
    
    daa.register_agent(
        Agent::new("risk_manager")
            .with_handler(Box::new(RiskManagementAgent::new()))
            .with_capabilities(vec!["risk_assessment", "position_sizing"])
    ).await?;
    
    Ok(())
}
```

## Phase 4: MCP Server Integration (Day 4)

### 4.1 Enable DAA's Built-in MCP
```rust
use daa_orchestrator::mcp::{MpcServer, Tool};

pub async fn setup_mcp_tools(daa: &mut DaaOrchestrator) -> Result<()> {
    // DAA includes MCP - just configure it!
    let mcp_server = daa.mcp_server_mut();
    
    // Add custom trading tools
    mcp_server.register_tool(Tool {
        name: "query_market_data",
        description: "Query TimescaleDB for market data",
        handler: Box::new(MarketDataTool::new()),
    })?;
    
    mcp_server.register_tool(Tool {
        name: "execute_trade",
        description: "Execute trading decision",
        handler: Box::new(TradingExecutor::new()),
    })?;
    
    // Start MCP server (built into DAA)
    mcp_server.start().await?;
    Ok(())
}
```

## Phase 5: Rules Engine Integration (Day 5)

### 5.1 Define Trading Rules
```rust
use daa_orchestrator::rules::{Rule, RuleSet, Context};

pub fn create_trading_rules() -> RuleSet {
    RuleSet::new("trading_rules")
        .add_rule(Rule::new("max_position_size")
            .when(|ctx: &Context| ctx.get("position_size")? > 0.1)
            .then(|ctx: &mut Context| {
                ctx.set("action", "reject");
                ctx.set("reason", "Position size exceeds 10% limit");
            })
        )
        .add_rule(Rule::new("risk_check")
            .when(|ctx: &Context| ctx.get("risk_score")? > 0.8)
            .then(|ctx: &mut Context| {
                ctx.set("action", "reduce_position");
                ctx.set("adjustment", 0.5);
            })
        )
}
```

### 5.2 Apply Rules in DAA
```rust
pub async fn configure_rules_engine(daa: &mut DaaOrchestrator) -> Result<()> {
    let rules = create_trading_rules();
    daa.rules_engine().load_ruleset(rules)?;
    
    // Rules automatically apply to all decisions
    Ok(())
}
```

## Phase 6: Full Integration (Day 6-7)

### 6.1 Main Application Rewrite
```rust
// src/main.rs - Use REAL DAA
use daa_orchestrator::DaaOrchestrator;

#[tokio::main]
async fn main() -> Result<()> {
    // Load config
    let config = PlatformConfig::load()?;
    
    // Initialize REAL DAA orchestrator
    let mut daa = initialize_real_daa(&config).await?;
    
    // Set up components using DAA
    setup_trading_agents(&mut daa).await?;
    setup_mcp_tools(&mut daa).await?;
    configure_rules_engine(&mut daa).await?;
    
    // Connect our neural system to DAA
    let neural_integration = NeuralDaaIntegration::new(&daa).await?;
    
    // Start DAA orchestrator (handles everything)
    daa.start().await?;
    
    // DAA handles the event loop, coordination, etc.
    signal::ctrl_c().await?;
    daa.shutdown().await?;
    
    Ok(())
}
```

### 6.2 Migration Checklist
- [ ] Remove our custom DaaOrchestrator
- [ ] Remove our custom DataPipeline  
- [ ] Remove our custom Agent/Decision types
- [ ] Use DAA's event bus instead of direct calls
- [ ] Use DAA's data pipeline
- [ ] Enable DAA's MCP server
- [ ] Configure DAA's rules engine
- [ ] Update tests to use real DAA

## Phase 7: Testing & Validation (Day 8)

### 7.1 Integration Tests
```rust
#[tokio::test]
async fn test_real_daa_integration() {
    // Test with actual DAA components
    let daa = DaaOrchestrator::new_test().await.unwrap();
    
    // Verify event handling
    let event = DaaEvent::MarketDataReceived(test_data());
    daa.emit(event).await.unwrap();
    
    // Verify pipeline processing
    let result = daa.pipeline("trading_pipeline")
        .process(test_data())
        .await
        .unwrap();
    
    assert!(result.predictions.len() > 0);
}
```

## Key Benefits of Using Real DAA

1. **Built-in Event System**: No need for our custom implementation
2. **Data Pipeline**: Production-ready streaming data processing
3. **MCP Integration**: Already included, just configure
4. **Rules Engine**: Dynamic trading rules without code changes
5. **Monitoring**: Prometheus/Grafana integration built-in
6. **Deployment**: Docker Compose configs provided
7. **Multi-Agent Coordination**: Proven patterns for agent communication

## Migration Priority

1. **CRITICAL**: Use real `daa_orchestrator::DaaOrchestrator`
2. **HIGH**: Migrate to DAA's event-driven architecture
3. **HIGH**: Use DAA's data pipeline
4. **MEDIUM**: Enable MCP server
5. **MEDIUM**: Implement rules engine
6. **LOW**: Add economy manager (for future tokenomics)

## Success Criteria

- [ ] All agents registered with real DAA orchestrator
- [ ] Event-driven communication working
- [ ] Data pipeline processing market data
- [ ] MCP server accessible
- [ ] Rules engine evaluating decisions
- [ ] All tests passing with real DAA
- [ ] Performance targets still met

## Notes

- Our TimescaleDB can work with DAA (PostgreSQL compatible)
- Our Redis cache integrates seamlessly
- We can keep our FANN neural models
- GPT-4 can be used instead of Claude initially
- No blockchain integration needed initially