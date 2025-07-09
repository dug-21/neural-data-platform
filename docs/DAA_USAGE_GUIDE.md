# DAA (Distributed Autonomous Agents) Usage Guide

## Overview

The DAA integration provides autonomous, distributed decision-making capabilities for the neural-trader platform. Multiple specialized agents work together to analyze markets, assess risks, and make trading decisions through consensus mechanisms.

## Quick Start

### 1. Basic DAA Setup

```rust
use neural_trader::agents::DAABridge;
use neural_trader::config::DAAConfig;

// Configure DAA system
let config = DAAConfig {
    agent_count: 5,
    consensus_threshold: 0.7,  // 70% agreement required
    risk_tolerance: RiskLevel::Medium,
    decision_timeout: Duration::from_secs(5),
};

// Initialize DAA bridge
let daa = DAABridge::new(config)?;
```

### 2. Getting Trading Decisions

```rust
use neural_trader::data::MarketContext;

// Prepare market context
let context = MarketContext {
    symbol: "AAPL".to_string(),
    current_price: 150.25,
    volume: 1_000_000,
    volatility: 0.15,
    trend: TrendDirection::Upward,
    support_levels: vec![148.0, 145.0],
    resistance_levels: vec![152.0, 155.0],
};

// Get consensus decision
let decision = daa.get_consensus_decision("AAPL", &context)?;

match decision.action {
    Action::Buy => {
        println!("Buy {} shares of {}", decision.quantity, decision.symbol);
        println!("Confidence: {:.2}%", decision.confidence * 100.0);
    },
    Action::Sell => {
        println!("Sell {} shares of {}", decision.quantity, decision.symbol);
    },
    Action::Hold => {
        println!("Hold position");
    }
}
```

## Agent Types and Specializations

### 1. Risk Analyst Agent
Focuses on risk assessment and portfolio protection.

```rust
let risk_agent = daa.spawn_agent(AgentType::RiskAnalyst)?;
```

**Capabilities:**
- Value at Risk (VaR) calculation
- Stop-loss recommendations
- Position sizing based on risk
- Correlation analysis

### 2. Technical Analyst Agent
Analyzes technical indicators and chart patterns.

```rust
let tech_agent = daa.spawn_agent(AgentType::TechnicalAnalyst)?;
```

**Capabilities:**
- Moving average crossovers
- RSI, MACD, Bollinger Bands
- Support/resistance identification
- Pattern recognition

### 3. Fundamental Analyst Agent
Evaluates company fundamentals and market conditions.

```rust
let fundamental_agent = daa.spawn_agent(AgentType::FundamentalAnalyst)?;
```

**Capabilities:**
- P/E ratio analysis
- Earnings momentum
- Sector comparison
- Economic indicators

### 4. Sentiment Analyst Agent
Monitors market sentiment and news impact.

```rust
let sentiment_agent = daa.spawn_agent(AgentType::SentimentAnalyst)?;
```

**Capabilities:**
- News sentiment scoring
- Social media analysis
- Market fear/greed index
- Volume analysis

### 5. Arbitrageur Agent
Identifies arbitrage and mean reversion opportunities.

```rust
let arb_agent = daa.spawn_agent(AgentType::Arbitrageur)?;
```

**Capabilities:**
- Cross-exchange arbitrage
- Statistical arbitrage
- Pairs trading opportunities
- Mean reversion signals

## Advanced Usage

### Custom Agent Configuration

```rust
use neural_trader::agents::{AgentConfig, AgentPersonality};

let custom_config = AgentConfig {
    personality: AgentPersonality::Conservative,
    specialization: vec![
        Specialization::RiskManagement,
        Specialization::LongTermInvesting,
    ],
    decision_weight: 1.5,  // Higher weight in consensus
    update_frequency: Duration::from_secs(1),
};

let custom_agent = daa.spawn_custom_agent(custom_config)?;
```

### Consensus Mechanisms

#### Weighted Voting
```rust
// Configure weighted consensus
daa.set_consensus_method(ConsensusMethod::WeightedVoting {
    experience_weight: 0.3,
    performance_weight: 0.4,
    specialization_weight: 0.3,
})?;
```

#### Hierarchical Decision Making
```rust
// Set up hierarchical structure
daa.set_consensus_method(ConsensusMethod::Hierarchical {
    leader: risk_agent,
    validators: vec![tech_agent, fundamental_agent],
    executors: vec![sentiment_agent, arb_agent],
})?;
```

### Real-Time Monitoring

```rust
// Subscribe to agent decisions
let mut decision_stream = daa.subscribe_to_decisions()?;

while let Some(agent_decision) = decision_stream.next().await {
    println!("Agent {}: {} on {} (confidence: {:.2}%)",
        agent_decision.agent_id,
        agent_decision.action,
        agent_decision.symbol,
        agent_decision.confidence * 100.0
    );
}
```

### Performance Tracking

```rust
// Get agent performance metrics
let metrics = daa.get_agent_metrics(agent_id)?;

println!("Agent {} Performance:", agent_id);
println!("  Success Rate: {:.2}%", metrics.success_rate * 100.0);
println!("  Average Return: {:.2}%", metrics.avg_return * 100.0);
println!("  Sharpe Ratio: {:.2}", metrics.sharpe_ratio);
println!("  Total Decisions: {}", metrics.total_decisions);
```

## Integration with Neural Models

### Combining DAA with Neural Predictions

```rust
use neural_trader::neural::FANNPredictor;

// Get neural prediction
let predictor = FANNPredictor::new(neural_config)?;
let prediction = predictor.predict("AAPL", &market_data)?;

// Enhance market context with neural predictions
let enhanced_context = MarketContext {
    neural_prediction: Some(prediction),
    ..context
};

// DAA agents will consider neural predictions in their analysis
let decision = daa.get_consensus_decision("AAPL", &enhanced_context)?;
```

### Feedback Loop

```rust
// Report trade results back to DAA for learning
let trade_result = TradeResult {
    decision_id: decision.id,
    actual_return: 0.025,
    execution_price: 150.30,
    exit_price: 151.10,
    duration: Duration::from_hours(4),
};

daa.report_trade_result(trade_result)?;
```

## Configuration Options

### Environment Variables
```bash
# DAA Configuration
DAA_AGENT_COUNT=5
DAA_CONSENSUS_THRESHOLD=0.7
DAA_DECISION_TIMEOUT_MS=5000
DAA_RISK_TOLERANCE=medium

# Agent Spawning
DAA_AUTO_SPAWN_AGENTS=true
DAA_MIN_AGENTS=3
DAA_MAX_AGENTS=10

# Performance
DAA_PARALLEL_ANALYSIS=true
DAA_CACHE_DECISIONS=true
DAA_CACHE_TTL_SECONDS=60
```

### Configuration File (config/daa.yaml)
```yaml
daa:
  consensus:
    method: weighted_voting
    threshold: 0.7
    timeout_ms: 5000
  
  agents:
    default_count: 5
    types:
      - risk_analyst: 2
      - technical_analyst: 1
      - fundamental_analyst: 1
      - sentiment_analyst: 1
    
  risk_management:
    max_position_size: 0.1  # 10% of portfolio
    stop_loss_percentage: 0.02  # 2%
    take_profit_percentage: 0.05  # 5%
    
  learning:
    enable_feedback: true
    update_interval: 3600  # 1 hour
    min_samples: 100
```

## Best Practices

### 1. Agent Diversity
Always spawn agents with different specializations for balanced decisions:

```rust
// Good: Diverse agent types
let agents = vec![
    AgentType::RiskAnalyst,
    AgentType::TechnicalAnalyst,
    AgentType::FundamentalAnalyst,
    AgentType::SentimentAnalyst,
];

for agent_type in agents {
    daa.spawn_agent(agent_type)?;
}
```

### 2. Consensus Threshold
Set appropriate thresholds based on market conditions:

```rust
// High volatility: Require stronger consensus
if market_volatility > 0.3 {
    daa.set_consensus_threshold(0.8)?;  // 80% agreement
}

// Normal conditions
else {
    daa.set_consensus_threshold(0.6)?;  // 60% agreement
}
```

### 3. Error Handling
Always handle consensus timeouts gracefully:

```rust
match daa.get_consensus_decision(symbol, &context) {
    Ok(decision) => process_decision(decision),
    Err(DAAError::ConsensusTimeout) => {
        // Fall back to conservative action
        log::warn!("Consensus timeout, holding position");
        Action::Hold
    },
    Err(e) => return Err(e.into()),
}
```

### 4. Monitoring and Alerts
Set up monitoring for agent health:

```rust
// Monitor agent responsiveness
let health = daa.check_health()?;
if health.unresponsive_agents > 0 {
    alert!("DAA: {} agents unresponsive", health.unresponsive_agents);
    
    // Spawn replacement agents
    for _ in 0..health.unresponsive_agents {
        daa.spawn_agent(AgentType::RiskAnalyst)?;
    }
}
```

## Troubleshooting

### Common Issues

1. **Consensus Not Reached**
   - Check if enough agents are active
   - Verify consensus threshold isn't too high
   - Review market context data quality

2. **Slow Decision Making**
   - Reduce decision timeout
   - Enable parallel analysis
   - Check system resources

3. **Inconsistent Decisions**
   - Ensure agents have consistent market data
   - Check for network latency issues
   - Review agent configuration

### Debug Mode

Enable detailed logging:

```rust
// Enable DAA debug logging
env::set_var("RUST_LOG", "neural_trader::agents=debug");
env_logger::init();

// Get detailed decision reasoning
let decision = daa.get_consensus_decision_with_details(symbol, &context)?;
for (agent_id, reasoning) in decision.agent_reasoning {
    println!("Agent {}: {}", agent_id, reasoning);
}
```

## Performance Optimization

### 1. Caching Decisions
```rust
// Enable decision caching for repeated queries
daa.enable_decision_cache(Duration::from_secs(60))?;
```

### 2. Batch Processing
```rust
// Process multiple symbols at once
let symbols = vec!["AAPL", "GOOGL", "MSFT"];
let decisions = daa.get_batch_decisions(&symbols, &contexts)?;
```

### 3. Async Operations
```rust
// Non-blocking consensus gathering
let decision_future = daa.get_consensus_decision_async(symbol, &context);
let decision = decision_future.await?;
```

## Example: Complete Trading Bot with DAA

```rust
use neural_trader::{DAABridge, FANNPredictor, TradingSystem};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize components
    let daa = DAABridge::new(Default::default())?;
    let predictor = FANNPredictor::new(Default::default())?;
    let trading_system = TradingSystem::new()?;
    
    // Spawn diverse agents
    daa.spawn_agent(AgentType::RiskAnalyst)?;
    daa.spawn_agent(AgentType::TechnicalAnalyst)?;
    daa.spawn_agent(AgentType::SentimentAnalyst)?;
    
    // Main trading loop
    loop {
        for symbol in &["AAPL", "GOOGL", "MSFT"] {
            // Get market data
            let market_data = trading_system.get_market_data(symbol).await?;
            
            // Get neural prediction
            let prediction = predictor.predict(symbol, &market_data)?;
            
            // Prepare context
            let context = MarketContext::from_market_data(market_data)
                .with_neural_prediction(prediction);
            
            // Get DAA consensus
            let decision = daa.get_consensus_decision(symbol, &context)?;
            
            // Execute trade if confident
            if decision.confidence > 0.7 {
                trading_system.execute_decision(decision).await?;
            }
        }
        
        // Wait before next iteration
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
```

## Further Resources

- [Architecture Documentation](./ARCHITECTURE.md)
- [API Documentation](./API_DOCUMENTATION.md)
- [Neural Integration Guide](./NEURAL_MIGRATION_PLAN.md)
- [Configuration Reference](./CONFIGURATION.md)