# Revised Autonomous Platform Architecture

## Overview

This revised architecture leverages ruv-FANN and ruv-DAA libraries to avoid duplication and focuses only on building genuinely new functionality.

## Core Principle

**USE libraries, DON'T recreate them**

## Simplified Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Domain-Specific Applications                      │
│                    (Trading, IoT, Recommendations)                   │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         Integration Layer                            │
│                    (Domain Adapters & Glue Code)                    │
└─────────────────────────────────────────────────────────────────────┘
                                    │
        ┌───────────────────────┬───┴───┬─────────────────────┐
        ▼                       ▼       ▼                     ▼
┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│  ruv-swarm  │  │   ruv-DAA   │  │ TimescaleDB │  │    Redis    │
│    (MCP)    │  │   (Agents)  │  │   (Custom)  │  │  (Custom)   │
└─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘
        │                │                │                │
        └────────────────┴────────────────┴────────────────┘
                                    │
                         ┌──────────┴──────────┐
                         │     ruv-FANN        │
                         │  (Neural Models)    │
                         └─────────────────────┘
```

## What We Actually Build

### 1. Data Platform Layer (NEW)

```rust
// This is genuinely new - libraries don't provide this
pub mod data {
    pub struct TimescaleDBStorage {
        pool: PgPool,
    }

    pub struct RedisCache {
        pool: RedisPool,
    }

    pub struct DataPipeline {
        storage: Arc<TimescaleDBStorage>,
        cache: Arc<RedisCache>,
    }
}
```

### 2. Integration Layer (NEW)

```rust
// Glue code between libraries and our data platform
pub mod integration {
    use ruv_swarm_ml::ForecastingManager;
    use ruv_daa::orchestrator::DaaOrchestrator;
    
    pub struct PlatformIntegrator {
        forecasting: ForecastingManager,
        orchestrator: DaaOrchestrator,
        data_pipeline: DataPipeline,
    }
}
```

### 3. Domain Adapters (NEW)

```rust
// Transform domain-specific data to/from library formats
pub trait DomainAdapter {
    type Input;
    type Output;
    
    fn adapt_for_neural(&self, input: Self::Input) -> Vec<f64>;
    fn adapt_from_decision(&self, decision: DaaDecision) -> Self::Output;
}
```

### 4. Custom MCP Tools (PARTIAL)

```rust
// Only add tools not in the 16 provided by ruv-swarm-mcp
pub fn register_custom_tools(mcp: &mut McpServer) {
    mcp.register_tool("query_timeseries", |params| {
        // Query our TimescaleDB
    });
    
    mcp.register_tool("invalidate_cache", |params| {
        // Manage Redis cache
    });
}
```

## What We DON'T Build

### ❌ Neural Networks
```rust
// DON'T DO THIS
pub struct CustomNHITS { ... }  // ❌ Use ruv-FANN
pub struct CustomDeepAR { ... } // ❌ Use ruv-FANN
```

**Instead:**
```rust
use ruv_swarm_ml::models::{NHITS, DeepAR, TCN, MLP};
```

### ❌ Agent Framework
```rust
// DON'T DO THIS
pub trait AutonomousAgent { ... }  // ❌ Use DAA
```

**Instead:**
```rust
use ruv_daa::agent::{DaaAgent, AgentCapability};
use ruv_daa::orchestrator::AutonomyLoop;
```

### ❌ MCP Server
```rust
// DON'T DO THIS
pub struct McpServer { ... }  // ❌ Use ruv-swarm-mcp
```

**Instead:**
```rust
use ruv_swarm_mcp::{McpServer, Tool};
```

## Dependency Configuration

```toml
[dependencies]
# Use the libraries!
ruv-fann = "0.1.3"
ruv-swarm-core = "0.2.0"
ruv-swarm-ml = "0.2.0"
ruv-swarm-mcp = "0.2.0"
ruv-swarm-transport = "0.2.0"
ruv-daa = { git = "https://github.com/ruvnet/daa.git", branch = "main" }

# Only what we actually need to add
sqlx = { version = "0.7", features = ["postgres", "runtime-tokio-rustls"] }
redis = { version = "0.25", features = ["tokio-comp"] }
tokio = { version = "1.39", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
config = "0.14"
anyhow = "1.0"
tracing = "0.1"
```

## File Structure (Minimal)

```
autonomous-platform/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── main.rs
│   ├── data/              # NEW: Our data platform
│   │   ├── mod.rs
│   │   ├── storage.rs     # TimescaleDB
│   │   └── cache.rs       # Redis
│   ├── integration/       # NEW: Library integration
│   │   ├── mod.rs
│   │   ├── neural.rs      # ruv-FANN integration
│   │   └── agents.rs      # ruv-DAA integration
│   └── adapters/          # NEW: Domain adapters
│       ├── mod.rs
│       └── base.rs
├── docker/
│   └── docker-compose.yml # TimescaleDB + Redis only
└── config/
    └── platform.toml
```

## Example Implementation

```rust
// src/main.rs
use ruv_swarm_ml::ForecastingManager;
use ruv_daa::orchestrator::DaaOrchestrator;
use ruv_swarm_mcp::McpServer;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initialize our custom data platform
    let data_platform = DataPlatform::new().await?;
    
    // 2. Use library components directly
    let forecasting = ForecastingManager::new(1024.0)?; // 1GB memory
    let orchestrator = DaaOrchestrator::builder()
        .with_topology(Topology::Hierarchical)
        .build()?;
    let mcp_server = McpServer::new()?;
    
    // 3. Wire them together with minimal glue code
    let platform = PlatformIntegrator {
        forecasting,
        orchestrator,
        data_platform,
        mcp_server,
    };
    
    platform.start().await
}
```

## Benefits of This Approach

1. **70% Less Code**: ~3,000 lines instead of ~10,000
2. **More Features**: Access to 27+ models instead of 4
3. **Better Performance**: Libraries are optimized
4. **Faster Delivery**: 3 weeks instead of 6
5. **Future-Proof**: Benefit from library updates
6. **Less Maintenance**: Fewer bugs in less code

## Migration Path for Existing Code

If you have existing mock implementations:

1. **Delete** all neural network mocks
2. **Delete** all agent framework code
3. **Keep** only data integration code
4. **Add** thin adapters to connect libraries

## Conclusion

This revised architecture properly leverages the ruv-FANN and ruv-DAA ecosystems while focusing development effort on the genuinely new functionality: data platform integration and domain-specific adapters. The result is a simpler, more powerful, and more maintainable system.