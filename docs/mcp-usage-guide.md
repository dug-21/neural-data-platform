# Universal Discovery Platform MCP Interface Usage Guide

## Overview

This guide demonstrates how to use the Universal Discovery Platform's MCP (Model Context Protocol) interface effectively. The interface is designed to be intuitive for both Claude AI and human users, with natural language support for AI agents and structured parameters for human analysts.

## Tool Categories & Rate Limits

| Category | Tools | Rate Limit | Permission Level |
|----------|-------|------------|------------------|
| **Discovery** | 5 tools | 100/hour | Standard |
| **Monitoring** | 4 tools | 1000/hour | Standard |
| **Execution** | 4 tools | 500/hour | Elevated |
| **System** | 4 tools | 50/hour | Admin |

## Discovery Tools

### 1. discover_market_connections

**Purpose**: Find hidden correlations and connections between markets, assets, or data sources.

**Claude Usage Example**:
```json
{
  "query": "Find connections between crypto fear index and traditional equity volatility",
  "timeframe": "6M",
  "confidence_threshold": 0.75,
  "discovery_mode": "guided"
}
```

**Human Usage Example**:
```json
{
  "assets": ["BTC", "SPY", "VIX", "GOLD"],
  "correlation_types": ["pearson", "spearman", "mutual_info"],
  "timeframe": "1Y",
  "confidence_threshold": 0.8,
  "include_lagged": true,
  "max_lag_days": 30
}
```

**Response Example**:
```json
{
  "connections": [
    {
      "source": "BTC",
      "target": "NASDAQ",
      "correlation": 0.82,
      "lag_days": 2,
      "confidence": 0.91,
      "strength": "strong",
      "type": "leading_indicator"
    }
  ],
  "discovery_insights": [
    "Bitcoin shows strong leading correlation with NASDAQ tech stocks",
    "2-day lag provides predictive edge with 91% confidence"
  ]
}
```

### 2. test_hypothesis

**Purpose**: Test specific trading or market hypotheses with statistical rigor.

**Claude Usage Example**:
```json
{
  "hypothesis": "RSI oversold signals in crypto provide 15%+ returns within 7 days",
  "test_period": "2Y",
  "parameters": {
    "rsi_threshold": 30,
    "holding_period": 7,
    "asset_class": "cryptocurrency"
  },
  "validation_method": "walk_forward"
}
```

**Human Usage Example**:
```json
{
  "hypothesis": "High VIX correlates with increased gold demand",
  "test_period": "5Y",
  "parameters": {
    "vix_threshold": 25,
    "correlation_window": 30
  },
  "validation_method": "bootstrap",
  "significance_level": 0.05
}
```

### 3. analyze_correlation

**Purpose**: Perform deep correlation analysis with multiple statistical methods.

**Example Usage**:
```json
{
  "datasets": ["SPY", "BTC", "GOLD", "VIX", "DXY"],
  "methods": ["pearson", "spearman", "mutual_info"],
  "time_window": 60,
  "include_partial_correlations": true,
  "regime_analysis": true
}
```

### 4. find_patterns

**Purpose**: Identify recurring patterns using machine learning.

**Claude Usage Example**:
```json
{
  "data_sources": ["BTC/USD", "ETH/USD", "SPY"],
  "pattern_types": ["technical", "seasonal"],
  "lookback_period": "2Y",
  "min_pattern_frequency": 3,
  "confidence_threshold": 0.8
}
```

### 5. predict_outcomes

**Purpose**: Generate predictions with confidence intervals.

**Example Usage**:
```json
{
  "target_variable": "BTC_price_1d",
  "prediction_horizon": "1D",
  "input_features": ["volume", "sentiment", "vix", "dxy"],
  "model_type": "lstm",
  "confidence_intervals": true
}
```

## Monitoring Tools

### 6. watch_markets

**Purpose**: Set up real-time monitoring with alerts.

**Example Usage**:
```json
{
  "watch_name": "Crypto Volatility Monitor",
  "conditions": [
    {
      "asset": "BTC",
      "condition_type": "volatility_change",
      "threshold": 0.15,
      "direction": "above"
    }
  ],
  "notification_settings": {
    "webhook_url": "https://myapp.com/alerts",
    "priority": "high"
  },
  "update_frequency": "realtime"
}
```

### 7. track_performance

**Purpose**: Monitor strategy and discovery performance.

**Example Usage**:
```bash
GET /monitoring/track-performance?strategy_id=momentum_v1&timeframe=1M&include_attribution=true
```

**Response**:
```json
{
  "returns": {
    "total_return": 0.085,
    "annualized_return": 0.102,
    "sharpe_ratio": 1.45,
    "hit_rate": 0.68
  },
  "risk_metrics": {
    "volatility": 0.12,
    "max_drawdown": 0.08,
    "var_95": 0.025
  }
}
```

### 8. get_alerts

**Purpose**: Retrieve current alerts and notifications.

**Example Usage**:
```bash
GET /monitoring/alerts?priority=high&status=active&limit=20
```

### 9. view_discoveries

**Purpose**: View recent AI-generated discoveries.

**Example Usage**:
```bash
GET /monitoring/discoveries?category=opportunities&confidence_min=0.8&timeframe=1D
```

## Execution Tools

### 10. create_strategy

**Purpose**: Create new trading strategies from discovered patterns.

**Example Usage**:
```json
{
  "name": "Crypto Momentum Strategy",
  "strategy_type": "momentum",
  "description": "Based on discovered BTC-NASDAQ correlation",
  "rules": [
    {
      "name": "Entry Rule",
      "type": "entry",
      "conditions": ["BTC_momentum > 0.1", "NASDAQ_sentiment > 0.6"],
      "actions": ["long_crypto_basket"]
    }
  ],
  "risk_parameters": {
    "max_position_size": 0.1,
    "stop_loss": 0.05,
    "max_drawdown": 0.15
  },
  "universe": ["BTC", "ETH", "ADA"]
}
```

### 11. deploy_rule

**Purpose**: Deploy specific trading rules.

**Example Usage**:
```json
{
  "rule_name": "Volatility Risk Management",
  "rule_type": "risk_management",
  "conditions": ["portfolio_volatility > 0.2"],
  "actions": ["reduce_positions_by_20_percent"],
  "priority": 9,
  "active": true
}
```

### 12. execute_trade

**Purpose**: Execute trading orders.

**Example Usage**:
```json
{
  "asset": "BTC",
  "side": "buy",
  "quantity": 0.5,
  "order_type": "limit",
  "limit_price": 45000,
  "time_in_force": "day",
  "execution_algorithm": "stealth"
}
```

### 13. manage_portfolio

**Purpose**: Get portfolio overview and recommendations.

**Example Usage**:
```bash
GET /execution/portfolio?include_recommendations=true&risk_level=moderate
```

## System Tools

### 14. configure_streams

**Purpose**: Configure data streams and feeds.

**Example Usage**:
```json
{
  "stream_name": "Social Sentiment Feed",
  "data_source": "social_sentiment",
  "provider": "twitter_api",
  "configuration": {
    "keywords": ["bitcoin", "crypto", "btc"],
    "languages": ["en"],
    "sentiment_model": "finbert"
  },
  "update_frequency": "realtime"
}
```

### 15. manage_domains

**Purpose**: Configure discovery domains.

**Example Usage**:
```json
{
  "domain_name": "cryptocurrencies",
  "action": "configure",
  "configuration": {
    "assets": ["BTC", "ETH", "ADA", "DOT"],
    "data_sources": ["price", "volume", "sentiment", "onchain"],
    "discovery_sensitivity": 0.8
  }
}
```

### 16. set_parameters

**Purpose**: Update system parameters.

**Example Usage**:
```json
{
  "discovery_parameters": {
    "sensitivity": 0.8,
    "confidence_threshold": 0.75
  },
  "risk_parameters": {
    "max_var": 0.05,
    "max_drawdown": 0.15,
    "concentration_limit": 0.2
  },
  "execution_parameters": {
    "slippage_tolerance": 0.001,
    "market_impact_limit": 0.005
  }
}
```

### 17. control_scaling

**Purpose**: Manage system scaling.

**Example Usage**:
```json
{
  "scaling_action": "auto_scale",
  "target_capacity": 80,
  "components": ["discovery_engine", "data_processing"]
}
```

## Claude AI Integration Examples

### Natural Language Discovery
```python
# Claude can use natural language queries
claude_query = {
  "tool": "discover_market_connections",
  "query": "I noticed unusual crypto behavior yesterday. Can you find what traditional markets might be influencing this pattern?"
}
```

### Hypothesis Testing
```python
# Claude can formulate and test hypotheses
claude_hypothesis = {
  "tool": "test_hypothesis", 
  "hypothesis": "When VIX spikes above 30, crypto markets tend to follow equity markets down with a 1-2 day lag",
  "test_period": "3Y"
}
```

### Pattern Recognition
```python
# Claude can identify complex patterns
claude_pattern = {
  "tool": "find_patterns",
  "query": "Find patterns that occur before major crypto rallies, especially ones that involve traditional market signals"
}
```

## Human Analyst Workflows

### Daily Discovery Routine
1. **Check Discoveries**: `view_discoveries` for overnight findings
2. **Review Alerts**: `get_alerts` for active notifications  
3. **Track Performance**: `track_performance` for strategy monitoring
4. **Test Ideas**: `test_hypothesis` for new trading ideas

### Strategy Development Process
1. **Discover Patterns**: `find_patterns` on historical data
2. **Test Hypothesis**: `test_hypothesis` with discovered patterns
3. **Create Strategy**: `create_strategy` based on validated patterns
4. **Deploy Rules**: `deploy_rule` for execution
5. **Monitor Performance**: `track_performance` ongoing

### Risk Management Workflow
1. **Monitor Markets**: `watch_markets` for risk events
2. **Track Portfolio**: `manage_portfolio` for exposure
3. **Set Parameters**: `set_parameters` for risk limits
4. **Scale System**: `control_scaling` for capacity

## Error Handling

### Common Error Responses

**Rate Limit Exceeded**:
```json
{
  "code": "RATE_LIMIT_EXCEEDED",
  "message": "Discovery tools rate limit exceeded. Limit: 100 requests/hour",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

**Insufficient Permissions**:
```json
{
  "code": "INSUFFICIENT_PERMISSIONS", 
  "message": "User lacks required permission: hypothesis_testing",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

**Invalid Parameters**:
```json
{
  "code": "INVALID_PARAMETERS",
  "message": "Required parameter 'assets' is missing",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

## Best Practices

### For Claude AI
- Use natural language queries for exploration
- Leverage guided discovery mode for open-ended research
- Chain multiple tools for comprehensive analysis
- Use predictions to validate discovered patterns

### For Human Users
- Start with low-confidence thresholds for exploration
- Use higher thresholds for execution decisions
- Combine multiple correlation methods for robustness
- Monitor performance continuously after deployment

### General Guidelines
- Respect rate limits by batching related queries
- Use appropriate timeframes for analysis
- Validate discoveries with multiple methods
- Set conservative risk parameters initially
- Monitor system health and scaling needs

## Integration Examples

### Python SDK Example
```python
import requests

class DiscoveryPlatform:
    def __init__(self, api_key):
        self.api_key = api_key
        self.base_url = "https://api.neural-trader.ai/v1"
        
    def discover_connections(self, query, timeframe="6M"):
        return requests.post(
            f"{self.base_url}/discovery/market-connections",
            headers={"X-API-Key": self.api_key},
            json={"query": query, "timeframe": timeframe}
        ).json()
        
    def test_hypothesis(self, hypothesis, test_period="2Y"):
        return requests.post(
            f"{self.base_url}/discovery/test-hypothesis", 
            headers={"X-API-Key": self.api_key},
            json={"hypothesis": hypothesis, "test_period": test_period}
        ).json()

# Usage
platform = DiscoveryPlatform("your-api-key")
connections = platform.discover_connections("Find crypto-equity correlations")
```

### Claude MCP Integration
```json
{
  "mcp_tool": "universal_discovery_discover_market_connections",
  "parameters": {
    "query": "Analyze relationship between DXY strength and emerging market performance",
    "timeframe": "1Y",
    "confidence_threshold": 0.8
  }
}
```

This interface provides a comprehensive yet intuitive way for both AI agents and human analysts to discover, monitor, and act on market opportunities using advanced pattern recognition and correlation analysis.