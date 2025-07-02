# API Specifications

## Overview

The Neural Trading Platform exposes multiple APIs for different components:
1. **Trading API** - RESTful API for trading operations
2. **MCP (Model Context Protocol)** - WebSocket API for AI agent coordination
3. **Neural Engine API** - Internal API for neural network operations
4. **Market Data API** - Real-time and historical market data access

## 1. Trading API (RESTful)

Base URL: `http://localhost:8081/api/v1`

### Authentication
```http
Authorization: Bearer <jwt_token>
```

### Core Endpoints

#### Account Management

**GET /account**
```json
{
  "account_id": "account_123",
  "name": "Trading Account",
  "status": "active",
  "buying_power": 98750.50,
  "cash": 25000.00,
  "portfolio_value": 125750.50,
  "day_trade_count": 2,
  "pattern_day_trader": false
}
```

**GET /account/portfolio**
```json
{
  "account_id": "account_123",
  "total_value": 125750.50,
  "cash": 25000.00,
  "positions_value": 100750.50,
  "unrealized_pnl": 2750.50,
  "realized_pnl_today": 450.00,
  "positions": [
    {
      "symbol": "AAPL",
      "quantity": 100,
      "side": "long",
      "market_value": 15025.00,
      "cost_basis": 14500.00,
      "unrealized_pnl": 525.00,
      "entry_price": 145.00,
      "current_price": 150.25
    }
  ]
}
```

#### Order Management

**POST /orders**
```json
// Request
{
  "symbol": "AAPL",
  "side": "buy",
  "type": "market",
  "quantity": 100,
  "time_in_force": "day",
  "client_order_id": "my_order_123"
}

// Response
{
  "id": "order_uuid_123",
  "client_order_id": "my_order_123",
  "status": "pending_new",
  "symbol": "AAPL",
  "side": "buy",
  "type": "market",
  "quantity": 100,
  "filled_quantity": 0,
  "remaining_quantity": 100,
  "created_at": "2024-01-15T14:30:00Z",
  "updated_at": "2024-01-15T14:30:00Z"
}
```

**GET /orders**
```json
{
  "orders": [
    {
      "id": "order_uuid_123",
      "status": "filled",
      "symbol": "AAPL",
      "side": "buy",
      "type": "market",
      "quantity": 100,
      "filled_quantity": 100,
      "average_fill_price": 150.25,
      "created_at": "2024-01-15T14:30:00Z",
      "filled_at": "2024-01-15T14:30:15Z"
    }
  ],
  "pagination": {
    "page": 1,
    "per_page": 50,
    "total": 1
  }
}
```

**GET /orders/{order_id}**
```json
{
  "id": "order_uuid_123",
  "status": "filled",
  "symbol": "AAPL",
  "side": "buy",
  "type": "market",
  "quantity": 100,
  "filled_quantity": 100,
  "remaining_quantity": 0,
  "average_fill_price": 150.25,
  "executions": [
    {
      "id": "exec_uuid_456",
      "quantity": 100,
      "price": 150.25,
      "timestamp": "2024-01-15T14:30:15Z",
      "venue": "NASDAQ"
    }
  ]
}
```

**DELETE /orders/{order_id}**
```json
{
  "id": "order_uuid_123",
  "status": "cancelled",
  "cancelled_at": "2024-01-15T14:35:00Z"
}
```

#### Market Data

**GET /market/quote/{symbol}**
```json
{
  "symbol": "AAPL",
  "bid": 150.20,
  "ask": 150.25,
  "bid_size": 500,
  "ask_size": 300,
  "last_price": 150.22,
  "last_size": 100,
  "volume": 1250000,
  "timestamp": "2024-01-15T14:30:00Z"
}
```

**GET /market/bars/{symbol}**
Query Parameters:
- `timeframe`: 1m, 5m, 15m, 1h, 1d
- `start`: ISO 8601 timestamp
- `end`: ISO 8601 timestamp
- `limit`: integer (max 1000)

```json
{
  "symbol": "AAPL",
  "timeframe": "1m",
  "bars": [
    {
      "timestamp": "2024-01-15T14:30:00Z",
      "open": 150.00,
      "high": 150.30,
      "low": 149.95,
      "close": 150.25,
      "volume": 15000
    }
  ]
}
```

#### DAA Agent Management

**GET /agents**
```json
{
  "agents": [
    {
      "id": "market_analyzer_1",
      "type": "market_analyzer",
      "status": "active",
      "health_score": 0.95,
      "last_heartbeat": "2024-01-15T14:30:00Z",
      "performance": {
        "predictions_count": 1250,
        "accuracy": 0.72,
        "avg_latency_ms": 3.2
      }
    },
    {
      "id": "risk_manager_1", 
      "type": "risk_manager",
      "status": "active",
      "health_score": 0.98,
      "last_heartbeat": "2024-01-15T14:30:00Z",
      "performance": {
        "decisions_count": 450,
        "risk_score": 0.35,
        "avg_latency_ms": 8.1
      }
    }
  ]
}
```

**POST /agents/{agent_id}/action**
```json
// Request
{
  "action": "restart",
  "parameters": {
    "preserve_state": true
  }
}

// Response
{
  "agent_id": "market_analyzer_1",
  "action": "restart",
  "status": "success",
  "message": "Agent restarted successfully"
}
```

#### Risk Management

**GET /risk/portfolio**
```json
{
  "account_id": "account_123",
  "timestamp": "2024-01-15T14:30:00Z",
  "metrics": {
    "var_95": 2500.00,
    "var_99": 4200.00,
    "expected_shortfall": 3100.00,
    "beta": 1.15,
    "sharpe_ratio": 1.42,
    "max_drawdown": 0.08
  },
  "limits": {
    "max_daily_loss": 5000.00,
    "max_position_size": 15000.00,
    "max_portfolio_var": 7500.00
  },
  "current_exposure": {
    "gross_exposure": 125750.50,
    "net_exposure": 100750.50,
    "leverage": 1.26
  }
}
```

**GET /risk/positions**
```json
{
  "positions": [
    {
      "symbol": "AAPL",
      "quantity": 100,
      "market_value": 15025.00,
      "var_95": 450.00,
      "beta": 1.20,
      "correlation_to_portfolio": 0.65,
      "risk_contribution": 0.18
    }
  ]
}
```

### Error Responses

All errors follow this format:
```json
{
  "error": {
    "code": "INVALID_ORDER",
    "message": "Order quantity must be positive",
    "details": {
      "field": "quantity",
      "value": -100
    },
    "timestamp": "2024-01-15T14:30:00Z"
  }
}
```

Common error codes:
- `UNAUTHORIZED` (401)
- `FORBIDDEN` (403)
- `NOT_FOUND` (404)
- `INVALID_ORDER` (400)
- `INSUFFICIENT_FUNDS` (400)
- `MARKET_CLOSED` (400)
- `RATE_LIMITED` (429)
- `INTERNAL_ERROR` (500)

## 2. MCP (Model Context Protocol) API

WebSocket endpoint: `ws://localhost:8080/mcp`

### Message Format

All MCP messages follow this structure:
```json
{
  "jsonrpc": "2.0",
  "method": "method_name",
  "params": { ... },
  "id": "unique_request_id"
}
```

### Tool Definitions

#### Trading Tools

**execute_trade**
```json
{
  "name": "execute_trade",
  "description": "Execute a trading order",
  "inputSchema": {
    "type": "object",
    "properties": {
      "symbol": {"type": "string"},
      "side": {"type": "string", "enum": ["buy", "sell"]},
      "quantity": {"type": "number"},
      "order_type": {"type": "string", "enum": ["market", "limit"]},
      "price": {"type": "number", "optional": true}
    },
    "required": ["symbol", "side", "quantity", "order_type"]
  }
}
```

**get_portfolio**
```json
{
  "name": "get_portfolio",
  "description": "Get current portfolio status",
  "inputSchema": {
    "type": "object",
    "properties": {
      "account_id": {"type": "string", "optional": true}
    }
  }
}
```

**analyze_market**
```json
{
  "name": "analyze_market",
  "description": "Analyze market conditions for a symbol",
  "inputSchema": {
    "type": "object",
    "properties": {
      "symbol": {"type": "string"},
      "timeframe": {"type": "string", "enum": ["1m", "5m", "15m", "1h", "1d"]},
      "lookback_periods": {"type": "integer", "default": 50}
    },
    "required": ["symbol"]
  }
}
```

#### Neural Network Tools

**neural_predict**
```json
{
  "name": "neural_predict",
  "description": "Get neural network prediction",
  "inputSchema": {
    "type": "object",
    "properties": {
      "model_name": {"type": "string"},
      "input_data": {"type": "array", "items": {"type": "number"}},
      "prediction_horizon": {"type": "integer", "default": 1}
    },
    "required": ["model_name", "input_data"]
  }
}
```

**retrain_model**
```json
{
  "name": "retrain_model", 
  "description": "Retrain a neural network model",
  "inputSchema": {
    "type": "object",
    "properties": {
      "model_id": {"type": "string"},
      "training_config": {"type": "object"},
      "dataset_filter": {"type": "object", "optional": true}
    },
    "required": ["model_id"]
  }
}
```

#### Risk Management Tools

**assess_risk**
```json
{
  "name": "assess_risk",
  "description": "Assess risk for a proposed trade",
  "inputSchema": {
    "type": "object", 
    "properties": {
      "symbol": {"type": "string"},
      "side": {"type": "string", "enum": ["buy", "sell"]},
      "quantity": {"type": "number"},
      "current_portfolio": {"type": "object", "optional": true}
    },
    "required": ["symbol", "side", "quantity"]
  }
}
```

**calculate_var**
```json
{
  "name": "calculate_var",
  "description": "Calculate Value at Risk",
  "inputSchema": {
    "type": "object",
    "properties": {
      "portfolio": {"type": "object"},
      "confidence_level": {"type": "number", "default": 0.95},
      "time_horizon": {"type": "integer", "default": 1}
    },
    "required": ["portfolio"]
  }
}
```

### Agent Communication

**agent_heartbeat**
```json
{
  "jsonrpc": "2.0",
  "method": "agent_heartbeat",
  "params": {
    "agent_id": "market_analyzer_1",
    "status": "active",
    "performance_metrics": {
      "latency_ms": 3.2,
      "accuracy": 0.72,
      "predictions_count": 1250
    },
    "timestamp": "2024-01-15T14:30:00Z"
  }
}
```

**agent_decision**
```json
{
  "jsonrpc": "2.0", 
  "method": "agent_decision",
  "params": {
    "agent_id": "risk_manager_1",
    "decision_type": "risk_assessment",
    "input_data": {
      "symbol": "AAPL",
      "proposed_quantity": 100,
      "current_portfolio": { ... }
    },
    "decision": {
      "action": "approve",
      "recommended_size": 100,
      "confidence": 0.85,
      "risk_score": 0.3
    },
    "reasoning": [
      "Position size within risk limits",
      "Low correlation with existing positions",
      "VaR impact acceptable"
    ]
  }
}
```

**coordination_request**
```json
{
  "jsonrpc": "2.0",
  "method": "coordination_request", 
  "params": {
    "requester_agent_id": "market_analyzer_1",
    "request_type": "analysis_input",
    "target_agents": ["risk_manager_1", "portfolio_manager_1"],
    "request_data": {
      "symbol": "AAPL",
      "signal": "bullish",
      "confidence": 0.78,
      "price_target": 155.00
    }
  }
}
```

## 3. Neural Engine API (Internal)

Base URL: `http://localhost:8082/neural`

### Model Management

**POST /models**
```json
// Request
{
  "name": "nhits_market_analyzer_v2",
  "type": "NHITS",
  "configuration": {
    "input_size": 50,
    "horizon": 24,
    "num_stacks": 3,
    "num_blocks_per_stack": 1
  },
  "training_config": {
    "learning_rate": 0.001,
    "batch_size": 32,
    "epochs": 100
  }
}

// Response
{
  "model_id": "model_uuid_123",
  "status": "created",
  "training_job_id": "training_job_456"
}
```

**GET /models/{model_id}/predict**
```json
// Request Query Parameters
// ?input=[150.25,149.80,150.10,...]&horizon=24

// Response
{
  "model_id": "model_uuid_123",
  "predictions": [150.35, 150.45, 150.30, ...],
  "confidence": 0.78,
  "latency_ms": 3.2,
  "timestamp": "2024-01-15T14:30:00Z"
}
```

**POST /models/{model_id}/train**
```json
// Request
{
  "dataset": {
    "start_date": "2024-01-01",
    "end_date": "2024-01-14",
    "symbols": ["AAPL", "GOOGL", "MSFT"],
    "features": ["price", "volume", "technical_indicators"]
  },
  "training_config": {
    "learning_rate": 0.001,
    "batch_size": 32,
    "epochs": 50,
    "validation_split": 0.2
  }
}

// Response
{
  "training_job_id": "training_job_789",
  "status": "started",
  "estimated_duration_minutes": 45
}
```

### Training Jobs

**GET /training/{job_id}**
```json
{
  "job_id": "training_job_789",
  "model_id": "model_uuid_123", 
  "status": "training",
  "progress": 0.65,
  "current_epoch": 32,
  "total_epochs": 50,
  "metrics": {
    "loss": 0.045,
    "val_loss": 0.052,
    "accuracy": 0.73,
    "val_accuracy": 0.69
  },
  "started_at": "2024-01-15T14:00:00Z",
  "estimated_completion": "2024-01-15T14:45:00Z"
}
```

## 4. Market Data API

Base URL: `http://localhost:8083/data`

### Real-time Data

**WebSocket: /stream**

Subscribe to real-time market data:
```json
{
  "action": "subscribe",
  "streams": ["quotes.AAPL", "trades.AAPL", "bars.AAPL.1m"]
}
```

Received messages:
```json
{
  "stream": "quotes.AAPL",
  "data": {
    "symbol": "AAPL", 
    "bid": 150.20,
    "ask": 150.25,
    "timestamp": "2024-01-15T14:30:00.123Z"
  }
}
```

### Historical Data

**GET /historical/bars**
Query Parameters:
- `symbols`: AAPL,GOOGL,MSFT
- `timeframe`: 1m, 5m, 15m, 1h, 1d
- `start`: 2024-01-01T00:00:00Z
- `end`: 2024-01-15T23:59:59Z
- `limit`: 1000

```json
{
  "bars": {
    "AAPL": [
      {
        "timestamp": "2024-01-15T14:30:00Z",
        "open": 150.00,
        "high": 150.30, 
        "low": 149.95,
        "close": 150.25,
        "volume": 15000
      }
    ]
  },
  "next_page_token": "eyJ0aW1lc3RhbXAiOiIyMDI0LTAxLTE1VDE0OjMwOjAwWiJ9"
}
```

## Rate Limiting

All APIs implement rate limiting:

### Trading API
- 100 requests per minute per user
- 10 orders per second per user
- Burst allowance: 20 requests

### MCP API
- 1000 messages per minute per connection
- No burst limit (real-time coordination)

### Neural Engine API
- 50 predictions per minute per model
- 5 training jobs per hour per user

### Market Data API
- 500 requests per minute per user
- WebSocket: 100 subscriptions per connection

Rate limit headers:
```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 85
X-RateLimit-Reset: 1642248600
```

## WebSocket Protocols

### Connection Authentication
```json
{
  "type": "auth",
  "token": "jwt_token_here"
}
```

### Heartbeat/Keepalive
```json
{
  "type": "ping",
  "timestamp": "2024-01-15T14:30:00Z"
}
```

Response:
```json
{
  "type": "pong", 
  "timestamp": "2024-01-15T14:30:00Z"
}
```

### Error Handling
```json
{
  "type": "error",
  "code": "SUBSCRIPTION_FAILED",
  "message": "Invalid symbol: INVALID_SYMBOL",
  "timestamp": "2024-01-15T14:30:00Z"
}
```

This comprehensive API specification provides the foundation for building client applications and integrating with the neural trading platform.