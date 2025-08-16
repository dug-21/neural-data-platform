# MCP Resource Definitions

## Overview

This document defines all MCP resources exposed by each layer of the Neural Trader system. Resources represent accessible data entities that can be retrieved, subscribed to, or manipulated through the MCP protocol.

## Resource URI Schemes

Each layer uses a specific URI scheme to organize and expose its resources:

- **Data Ingestion**: `ingestion://`
- **Discovery**: `discovery://`  
- **Analysis**: `analysis://`
- **Execution**: `execution://`
- **Storage**: `storage://`

## 1. Data Ingestion Layer Resources

### Stream Resources

#### `ingestion://streams/{provider}/{symbol}`
```json
{
  "uri": "ingestion://streams/{provider}/{symbol}",
  "name": "Real-time Data Stream",
  "description": "Live market data stream for a specific symbol from a provider",
  "mimeType": "application/json",
  "schema": {
    "type": "object",
    "properties": {
      "stream_id": {"type": "string"},
      "provider": {"type": "string"},
      "symbol": {"type": "string"},
      "data_type": {"type": "string", "enum": ["quotes", "trades", "ohlc", "news"]},
      "status": {"type": "string", "enum": ["active", "paused", "error"]},
      "metadata": {
        "type": "object",
        "properties": {
          "subscription_time": {"type": "string", "format": "date-time"},
          "last_message_time": {"type": "string", "format": "date-time"},
          "message_count": {"type": "integer"},
          "error_count": {"type": "integer"},
          "throughput_per_second": {"type": "number"},
          "latency_ms": {"type": "number"}
        }
      },
      "current_data": {
        "type": "object",
        "description": "Most recent data point received"
      }
    }
  },
  "permissions": ["read", "subscribe"],
  "rate_limits": {
    "read": "100/minute",
    "subscribe": "10/minute"
  }
}
```

#### `ingestion://streams/aggregated/{timeframe}`
```json
{
  "uri": "ingestion://streams/aggregated/{timeframe}",
  "name": "Aggregated Stream Data",
  "description": "Pre-aggregated market data streams by timeframe",
  "mimeType": "application/json",
  "schema": {
    "type": "object",
    "properties": {
      "timeframe": {"type": "string"},
      "symbols": {"type": "array", "items": {"type": "string"}},
      "aggregation_method": {"type": "string"},
      "data_points": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "symbol": {"type": "string"},
            "timestamp": {"type": "string"},
            "open": {"type": "number"},
            "high": {"type": "number"},
            "low": {"type": "number"},
            "close": {"type": "number"},
            "volume": {"type": "number"}
          }
        }
      }
    }
  },
  "permissions": ["read"],
  "cache_ttl": "1m"
}
```

### Provider Resources

#### `ingestion://providers/{provider}/status`
```json
{
  "uri": "ingestion://providers/{provider}/status",
  "name": "Provider Health Status",
  "description": "Health and status information for data providers",
  "mimeType": "application/json",
  "schema": {
    "type": "object",
    "properties": {
      "provider": {"type": "string"},
      "status": {"type": "string", "enum": ["online", "degraded", "offline"]},
      "uptime_percentage": {"type": "number"},
      "rate_limit_status": {
        "type": "object",
        "properties": {
          "current_usage": {"type": "integer"},
          "limit": {"type": "integer"},
          "reset_time": {"type": "string", "format": "date-time"}
        }
      },
      "error_metrics": {
        "type": "object",
        "properties": {
          "total_errors_24h": {"type": "integer"},
          "error_rate": {"type": "number"},
          "last_error": {"type": "string", "format": "date-time"}
        }
      },
      "performance_metrics": {
        "type": "object",
        "properties": {
          "avg_latency_ms": {"type": "number"},
          "p95_latency_ms": {"type": "number"},
          "throughput_per_second": {"type": "number"}
        }
      }
    }
  },
  "permissions": ["read"],
  "cache_ttl": "30s"
}
```

### Quality Metrics Resources

#### `ingestion://quality/metrics/{symbol}`
```json
{
  "uri": "ingestion://quality/metrics/{symbol}",
  "name": "Data Quality Metrics",
  "description": "Quality assessment metrics for ingested data",
  "mimeType": "application/json",
  "schema": {
    "type": "object",
    "properties": {
      "symbol": {"type": "string"},
      "quality_score": {"type": "number", "minimum": 0, "maximum": 1},
      "completeness": {"type": "number"},
      "timeliness": {"type": "number"},
      "accuracy": {"type": "number"},
      "consistency": {"type": "number"},
      "issues": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "type": {"type": "string"},
            "severity": {"type": "string"},
            "description": {"type": "string"},
            "first_observed": {"type": "string"},
            "frequency": {"type": "number"}
          }
        }
      }
    }
  },
  "permissions": ["read"],
  "cache_ttl": "5m"
}
```

## 2. Discovery Layer Resources

### Pattern Resources

#### `discovery://patterns/{pattern_id}`
```json
{
  "uri": "discovery://patterns/{pattern_id}",
  "name": "Discovered Pattern",
  "description": "Details of a specific pattern discovered in market data",
  "mimeType": "application/json",
  "schema": {
    "type": "object",
    "properties": {
      "pattern_id": {"type": "string"},
      "pattern_type": {
        "type": "string",
        "enum": ["support_resistance", "trend_line", "chart_pattern", "volume_pattern"]
      },
      "symbols": {"type": "array", "items": {"type": "string"}},
      "timeframe": {"type": "string"},
      "confidence": {"type": "number", "minimum": 0, "maximum": 1},
      "discovery_time": {"type": "string", "format": "date-time"},
      "pattern_data": {
        "type": "object",
        "properties": {
          "coordinates": {"type": "array"},
          "strength": {"type": "number"},
          "breakout_probability": {"type": "number"},
          "expected_move": {"type": "number"}
        }
      },
      "historical_performance": {
        "type": "object",
        "properties": {
          "success_rate": {"type": "number"},
          "avg_return": {"type": "number"},
          "max_drawdown": {"type": "number"},
          "sample_size": {"type": "integer"}
        }
      }
    }
  },
  "permissions": ["read"],
  "cache_ttl": "1h"
}
```

#### `discovery://patterns/active/{symbol}`
```json
{
  "uri": "discovery://patterns/active/{symbol}",
  "name": "Active Patterns for Symbol",
  "description": "Currently active patterns for a specific symbol",
  "mimeType": "application/json",
  "schema": {
    "type": "object",
    "properties": {
      "symbol": {"type": "string"},
      "active_patterns": {
        "type": "array",
        "items": {
          "$ref": "#/definitions/pattern"
        }
      },
      "pattern_summary": {
        "type": "object",
        "properties": {
          "bullish_count": {"type": "integer"},
          "bearish_count": {"type": "integer"},
          "neutral_count": {"type": "integer"},
          "overall_sentiment": {"type": "string"},
          "confidence_aggregate": {"type": "number"}
        }
      }
    }
  },
  "permissions": ["read"],
  "cache_ttl": "15m"
}
```

### Correlation Resources

#### `discovery://correlations/{correlation_id}`
```json
{
  "uri": "discovery://correlations/{correlation_id}",
  "name": "Correlation Analysis Result",
  "description": "Results of correlation analysis between assets",
  "mimeType": "application/json",
  "schema": {
    "type": "object",
    "properties": {
      "correlation_id": {"type": "string"},
      "primary_symbol": {"type": "string"},
      "correlated_symbols": {"type": "array", "items": {"type": "string"}},
      "analysis_timeframe": {"type": "string"},
      "correlation_matrix": {
        "type": "object",
        "additionalProperties": {
          "type": "object",
          "additionalProperties": {"type": "number"}
        }
      },
      "significant_correlations": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "symbol_pair": {"type": "array", "items": {"type": "string"}},
            "correlation": {"type": "number"},
            "significance": {"type": "number"},
            "relationship_type": {"type": "string"}
          }
        }
      },
      "dynamic_correlations": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "timestamp": {"type": "string"},
            "correlation": {"type": "number"},
            "rolling_window": {"type": "string"}
          }
        }
      }
    }
  },
  "permissions": ["read"],
  "cache_ttl": "30m"
}
```

### Causality Resources

#### `discovery://causality/{analysis_id}`
```json
{
  "uri": "discovery://causality/{analysis_id}",
  "name": "Causality Analysis Result",
  "description": "Results of causal relationship analysis",
  "mimeType": "application/json",
  "schema": {
    "type": "object",
    "properties": {
      "analysis_id": {"type": "string"},
      "method": {"type": "string"},
      "cause_variables": {"type": "array", "items": {"type": "string"}},
      "effect_variables": {"type": "array", "items": {"type": "string"}},
      "causal_relationships": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "cause": {"type": "string"},
            "effect": {"type": "string"},
            "strength": {"type": "number"},
            "lag": {"type": "integer"},
            "p_value": {"type": "number"},
            "confidence_interval": {"type": "array", "items": {"type": "number"}}
          }
        }
      },
      "network_graph": {
        "type": "object",
        "properties": {
          "nodes": {"type": "array"},
          "edges": {"type": "array"}
        }
      }
    }
  },
  "permissions": ["read"],
  "cache_ttl": "1h"
}
```

## 3. Analysis Layer Resources

### Decision Resources

#### `analysis://decisions/{decision_id}`
```json
{
  "uri": "analysis://decisions/{decision_id}",
  "name": "Trading Decision",
  "description": "Comprehensive trading decision with rationale",
  "mimeType": "application/json",
  "schema": {
    "type": "object",
    "properties": {
      "decision_id": {"type": "string"},
      "symbol": {"type": "string"},
      "decision": {"type": "string", "enum": ["buy", "sell", "hold", "reduce", "increase"]},
      "confidence": {"type": "number", "minimum": 0, "maximum": 1},
      "timestamp": {"type": "string", "format": "date-time"},
      "position_sizing": {
        "type": "object",
        "properties": {
          "recommended_size": {"type": "number"},
          "max_size": {"type": "number"},
          "risk_adjusted_size": {"type": "number"}
        }
      },
      "price_targets": {
        "type": "object",
        "properties": {
          "entry": {"type": "number"},
          "stop_loss": {"type": "number"},
          "take_profit_levels": {"type": "array", "items": {"type": "number"}}
        }
      },
      "rationale": {
        "type": "object",
        "properties": {
          "primary_factors": {"type": "array", "items": {"type": "string"}},
          "technical_analysis": {"type": "string"},
          "fundamental_analysis": {"type": "string"},
          "sentiment_analysis": {"type": "string"},
          "risk_assessment": {"type": "string"}
        }
      },
      "scenario_analysis": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "scenario": {"type": "string"},
            "probability": {"type": "number"},
            "expected_outcome": {"type": "string"},
            "recommended_action": {"type": "string"}
          }
        }
      },
      "model_inputs": {
        "type": "object",
        "description": "Inputs used for decision making"
      }
    }
  },
  "permissions": ["read"],
  "cache_ttl": "5m"
}
```

### Risk Assessment Resources

#### `analysis://risk/portfolio/{portfolio_id}`
```json
{
  "uri": "analysis://risk/portfolio/{portfolio_id}",
  "name": "Portfolio Risk Assessment",
  "description": "Comprehensive risk analysis for portfolio",
  "mimeType": "application/json",
  "schema": {
    "type": "object",
    "properties": {
      "portfolio_id": {"type": "string"},
      "assessment_time": {"type": "string", "format": "date-time"},
      "risk_metrics": {
        "type": "object",
        "properties": {
          "value_at_risk": {
            "type": "object",
            "properties": {
              "1d_95": {"type": "number"},
              "1d_99": {"type": "number"},
              "1w_95": {"type": "number"}
            }
          },
          "expected_shortfall": {"type": "number"},
          "max_drawdown": {"type": "number"},
          "volatility": {"type": "number"},
          "sharpe_ratio": {"type": "number"},
          "beta": {"type": "number"}
        }
      },
      "concentration_risk": {
        "type": "object",
        "properties": {
          "position_concentration": {"type": "array"},
          "sector_concentration": {"type": "array"},
          "geographic_concentration": {"type": "array"}
        }
      },
      "stress_tests": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "scenario": {"type": "string"},
            "portfolio_impact": {"type": "number"},
            "probability": {"type": "number"}
          }
        }
      },
      "recommendations": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "type": {"type": "string"},
            "description": {"type": "string"},
            "priority": {"type": "string"},
            "estimated_impact": {"type": "number"}
          }
        }
      }
    }
  },
  "permissions": ["read"],
  "cache_ttl": "15m"
}
```

### Strategy Resources

#### `analysis://strategies/{strategy_id}/performance`
```json
{
  "uri": "analysis://strategies/{strategy_id}/performance",
  "name": "Strategy Performance Analysis",
  "description": "Performance metrics and analysis for trading strategies",
  "mimeType": "application/json",
  "schema": {
    "type": "object",
    "properties": {
      "strategy_id": {"type": "string"},
      "strategy_name": {"type": "string"},
      "performance_period": {"type": "string"},
      "returns": {
        "type": "object",
        "properties": {
          "total_return": {"type": "number"},
          "annualized_return": {"type": "number"},
          "monthly_returns": {"type": "array", "items": {"type": "number"}},
          "cumulative_returns": {"type": "array"}
        }
      },
      "risk_metrics": {
        "type": "object",
        "properties": {
          "volatility": {"type": "number"},
          "max_drawdown": {"type": "number"},
          "var_95": {"type": "number"},
          "sharpe_ratio": {"type": "number"},
          "sortino_ratio": {"type": "number"}
        }
      },
      "trade_statistics": {
        "type": "object",
        "properties": {
          "total_trades": {"type": "integer"},
          "winning_trades": {"type": "integer"},
          "losing_trades": {"type": "integer"},
          "win_rate": {"type": "number"},
          "avg_win": {"type": "number"},
          "avg_loss": {"type": "number"},
          "profit_factor": {"type": "number"}
        }
      }
    }
  },
  "permissions": ["read"],
  "cache_ttl": "1h"
}
```

## 4. Execution Layer Resources

### Order Resources

#### `execution://orders/{order_id}`
```json
{
  "uri": "execution://orders/{order_id}",
  "name": "Order Details",
  "description": "Complete order information and execution details",
  "mimeType": "application/json",
  "schema": {
    "type": "object",
    "properties": {
      "order_id": {"type": "string"},
      "symbol": {"type": "string"},
      "side": {"type": "string", "enum": ["buy", "sell"]},
      "order_type": {"type": "string"},
      "status": {"type": "string", "enum": ["pending", "partial", "filled", "cancelled", "rejected"]},
      "quantity": {"type": "number"},
      "filled_quantity": {"type": "number"},
      "price": {"type": "number"},
      "average_fill_price": {"type": "number"},
      "created_time": {"type": "string", "format": "date-time"},
      "updated_time": {"type": "string", "format": "date-time"},
      "execution_details": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "execution_id": {"type": "string"},
            "quantity": {"type": "number"},
            "price": {"type": "number"},
            "timestamp": {"type": "string", "format": "date-time"},
            "venue": {"type": "string"},
            "commission": {"type": "number"}
          }
        }
      },
      "performance_metrics": {
        "type": "object",
        "properties": {
          "slippage": {"type": "number"},
          "execution_time": {"type": "number"},
          "implementation_shortfall": {"type": "number"}
        }
      }
    }
  },
  "permissions": ["read"],
  "cache_ttl": "1m"
}
```

#### `execution://orders/active`
```json
{
  "uri": "execution://orders/active",
  "name": "Active Orders",
  "description": "List of all active orders",
  "mimeType": "application/json",
  "schema": {
    "type": "object",
    "properties": {
      "active_orders": {
        "type": "array",
        "items": {
          "$ref": "#/definitions/order_summary"
        }
      },
      "summary": {
        "type": "object",
        "properties": {
          "total_active": {"type": "integer"},
          "total_value": {"type": "number"},
          "by_status": {"type": "object"},
          "by_symbol": {"type": "object"}
        }
      }
    }
  },
  "permissions": ["read"],
  "cache_ttl": "30s"
}
```

### Position Resources

#### `execution://positions/{symbol}`
```json
{
  "uri": "execution://positions/{symbol}",
  "name": "Position Details",
  "description": "Current position information for a symbol",
  "mimeType": "application/json",
  "schema": {
    "type": "object",
    "properties": {
      "symbol": {"type": "string"},
      "quantity": {"type": "number"},
      "side": {"type": "string", "enum": ["long", "short", "flat"]},
      "entry_price": {"type": "number"},
      "current_price": {"type": "number"},
      "market_value": {"type": "number"},
      "unrealized_pnl": {"type": "number"},
      "realized_pnl": {"type": "number"},
      "cost_basis": {"type": "number"},
      "last_updated": {"type": "string", "format": "date-time"},
      "position_history": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "timestamp": {"type": "string"},
            "action": {"type": "string"},
            "quantity": {"type": "number"},
            "price": {"type": "number"}
          }
        }
      }
    }
  },
  "permissions": ["read"],
  "cache_ttl": "1m"
}
```

### Neural Prediction Resources

#### `execution://predictions/{prediction_id}`
```json
{
  "uri": "execution://predictions/{prediction_id}",
  "name": "Neural Prediction Result",
  "description": "Results from neural network prediction execution",
  "mimeType": "application/json",
  "schema": {
    "type": "object",
    "properties": {
      "prediction_id": {"type": "string"},
      "symbol": {"type": "string"},
      "model_id": {"type": "string"},
      "prediction_time": {"type": "string", "format": "date-time"},
      "horizon": {"type": "integer"},
      "prediction": {
        "type": "object",
        "properties": {
          "value": {"type": "number"},
          "confidence": {"type": "number"},
          "direction": {"type": "string", "enum": ["up", "down", "sideways"]},
          "probability_distribution": {"type": "array"}
        }
      },
      "model_metadata": {
        "type": "object",
        "properties": {
          "model_version": {"type": "string"},
          "training_date": {"type": "string"},
          "accuracy_metrics": {"type": "object"},
          "feature_importance": {"type": "array"}
        }
      },
      "ensemble_details": {
        "type": "object",
        "properties": {
          "models_used": {"type": "array"},
          "individual_predictions": {"type": "array"},
          "consensus_strength": {"type": "number"}
        }
      }
    }
  },
  "permissions": ["read"],
  "cache_ttl": "5m"
}
```

## 5. Storage Layer Resources

### Data Resources

#### `storage://timeseries/{symbol}/{timeframe}`
```json
{
  "uri": "storage://timeseries/{symbol}/{timeframe}",
  "name": "Time Series Data",
  "description": "Historical time series data for a symbol",
  "mimeType": "application/json",
  "schema": {
    "type": "object",
    "properties": {
      "symbol": {"type": "string"},
      "timeframe": {"type": "string"},
      "data_points": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "timestamp": {"type": "string", "format": "date-time"},
            "open": {"type": "number"},
            "high": {"type": "number"},
            "low": {"type": "number"},
            "close": {"type": "number"},
            "volume": {"type": "number"}
          }
        }
      },
      "metadata": {
        "type": "object",
        "properties": {
          "start_date": {"type": "string"},
          "end_date": {"type": "string"},
          "total_points": {"type": "integer"},
          "gaps": {"type": "array"},
          "quality_score": {"type": "number"}
        }
      }
    }
  },
  "permissions": ["read"],
  "cache_ttl": "5m"
}
```

#### `storage://cache/{key}`
```json
{
  "uri": "storage://cache/{key}",
  "name": "Cached Data",
  "description": "Data stored in Redis cache",
  "mimeType": "application/json",
  "schema": {
    "type": "object",
    "properties": {
      "key": {"type": "string"},
      "value": {"type": "object"},
      "ttl": {"type": "integer"},
      "created_at": {"type": "string", "format": "date-time"},
      "last_accessed": {"type": "string", "format": "date-time"},
      "access_count": {"type": "integer"}
    }
  },
  "permissions": ["read", "write"],
  "cache_ttl": "dynamic"
}
```

### Model Resources

#### `storage://models/{model_id}`
```json
{
  "uri": "storage://models/{model_id}",
  "name": "Neural Network Model",
  "description": "Stored neural network model and metadata",
  "mimeType": "application/octet-stream",
  "schema": {
    "type": "object",
    "properties": {
      "model_id": {"type": "string"},
      "model_name": {"type": "string"},
      "model_type": {"type": "string"},
      "version": {"type": "string"},
      "created_date": {"type": "string", "format": "date-time"},
      "training_metadata": {
        "type": "object",
        "properties": {
          "training_data_size": {"type": "integer"},
          "training_duration": {"type": "string"},
          "validation_metrics": {"type": "object"},
          "hyperparameters": {"type": "object"}
        }
      },
      "model_artifact": {
        "type": "object",
        "properties": {
          "file_path": {"type": "string"},
          "file_size": {"type": "integer"},
          "checksum": {"type": "string"}
        }
      },
      "performance_history": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "date": {"type": "string"},
            "accuracy": {"type": "number"},
            "loss": {"type": "number"},
            "predictions_count": {"type": "integer"}
          }
        }
      }
    }
  },
  "permissions": ["read"],
  "cache_ttl": "1h"
}
```

### Analytics Resources

#### `storage://analytics/performance/{timeframe}`
```json
{
  "uri": "storage://analytics/performance/{timeframe}",
  "name": "Performance Analytics",
  "description": "System performance analytics and metrics",
  "mimeType": "application/json",
  "schema": {
    "type": "object",
    "properties": {
      "timeframe": {"type": "string"},
      "system_metrics": {
        "type": "object",
        "properties": {
          "cpu_utilization": {"type": "array"},
          "memory_usage": {"type": "array"},
          "disk_io": {"type": "array"},
          "network_io": {"type": "array"}
        }
      },
      "application_metrics": {
        "type": "object",
        "properties": {
          "request_latency": {"type": "array"},
          "throughput": {"type": "array"},
          "error_rates": {"type": "array"},
          "active_connections": {"type": "array"}
        }
      },
      "business_metrics": {
        "type": "object",
        "properties": {
          "trading_volume": {"type": "array"},
          "prediction_accuracy": {"type": "array"},
          "portfolio_returns": {"type": "array"}
        }
      }
    }
  },
  "permissions": ["read"],
  "cache_ttl": "10m"
}
```

## Resource Access Patterns

### Authentication and Authorization
```json
{
  "auth_config": {
    "method": "bearer_token",
    "scopes": ["read", "write", "subscribe", "admin"],
    "resource_permissions": {
      "ingestion://*": ["read", "subscribe"],
      "discovery://*": ["read"],
      "analysis://*": ["read"],
      "execution://orders/*": ["read", "write"],
      "storage://cache/*": ["read", "write"]
    }
  }
}
```

### Rate Limiting
```json
{
  "rate_limits": {
    "global": "1000/minute",
    "per_resource_type": {
      "ingestion": "100/minute",
      "discovery": "50/minute",
      "analysis": "25/minute",
      "execution": "10/minute",
      "storage": "200/minute"
    },
    "burst_allowance": 10
  }
}
```

### Caching Strategy
```json
{
  "caching": {
    "default_ttl": "5m",
    "cache_invalidation": "event_driven",
    "cache_levels": [
      "application_cache",
      "redis_cache",
      "cdn_cache"
    ],
    "cache_warming": {
      "enabled": true,
      "resources": ["storage://timeseries/*", "analysis://strategies/*/performance"]
    }
  }
}
```

## Resource Discovery

### Resource Catalog
```json
{
  "catalog": {
    "endpoint": "/.well-known/mcp-resources",
    "format": "json-ld",
    "include_schemas": true,
    "include_examples": true,
    "pagination": {
      "page_size": 50,
      "max_page_size": 200
    }
  }
}
```

This comprehensive resource definition ensures:
1. **Discoverability**: Clear URI schemes and catalog
2. **Type Safety**: Complete JSON schemas for all resources
3. **Performance**: Appropriate caching and rate limiting
4. **Security**: Proper authentication and authorization
5. **Monitoring**: Built-in metrics and analytics resources