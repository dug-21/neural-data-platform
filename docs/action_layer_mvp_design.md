# Neural Trader Action Layer MVP Design

## Overview

The Action Layer provides the minimal viable trading execution capabilities for the neural trader system. It bridges the gap between neural network predictions and actual trade execution while maintaining strict risk controls and audit compliance.

## Architecture Components

### 1. Core Action Layer (`mod.rs`)
- **Central Coordinator**: Orchestrates all trading operations
- **Configuration Management**: Paper trading vs live trading modes
- **Dependency Injection**: Manages broker, risk manager, position tracker instances
- **Emergency Controls**: Immediate stop/resume capabilities

### 2. Broker Integration (`brokers/`)

#### Alpaca Integration (`alpaca.rs`)
- **Paper Trading Broker**: Safe testing environment with simulated executions
- **Live Trading Broker**: Production-ready Alpaca API integration
- **Order Management**: Submit, cancel, and track orders
- **Account Sync**: Real-time account and position updates
- **Market Data**: Current price feeds for risk calculations

#### Paper Trading Broker (`paper_trading.rs`)
- **Simulated Execution**: Realistic order fills with market impact
- **Virtual Portfolio**: $100k starting capital for testing
- **Market Simulation**: Price movements with bid/ask spreads
- **Position Tracking**: Full P&L calculation without real money

### 3. Risk Management (`risk_manager.rs`)

#### Position Limits
- **Max Position Size**: 5% of portfolio per position (default)
- **Concentration Limits**: 30% maximum exposure to correlated assets
- **Portfolio Risk**: 10% maximum total portfolio risk

#### Daily Controls
- **Daily Loss Limit**: 2% maximum daily loss
- **Maximum Drawdown**: 15% portfolio protection
- **Stop Loss**: 2% default stop loss percentage

#### Dynamic Position Sizing
- **Kelly Criterion**: Optimized position sizing based on signal strength
- **Volatility Adjustment**: Risk-adjusted sizing based on market conditions
- **Liquidity Constraints**: Minimum $1000 position sizes

### 4. Position Tracking (`position_tracker.rs`)

#### Real-time P&L
- **Unrealized P&L**: Mark-to-market position valuation
- **Realized P&L**: Actual profits/losses from closed positions
- **Daily P&L**: Session-based performance tracking

#### Position Management
- **FIFO Accounting**: First-in-first-out position tracking
- **Average Entry Price**: Cost basis calculations with multiple fills
- **Position Lifecycle**: Creation, updates, and closure handling

### 5. Audit System (`audit_logger.rs`)

#### Comprehensive Logging
- **Order Events**: Submit, accept, fill, cancel, reject
- **Risk Events**: Violations, emergency stops, limit breaches
- **Position Updates**: Entry, exit, and P&L changes
- **System Events**: Startup, shutdown, configuration changes

#### Compliance Features
- **JSON Structured Logs**: Machine-readable audit trail
- **Session Tracking**: Unique session IDs for correlation
- **Error Recording**: Complete error context and stack traces

### 6. Emergency Controls (`emergency_controls.rs`)

#### Immediate Protection
- **Emergency Stop**: Instant halt of all trading activity
- **Order Cancellation**: Automatic cancel of pending orders
- **Risk Circuit Breaker**: Automatic stops on risk limit breaches
- **Manual Override**: API-controlled emergency interventions

#### Safety Features
- **Resume Validation**: Safety checks before resuming trading
- **Stop Count Tracking**: Circuit breaker for repeated stops
- **Historical Logging**: Complete emergency event history

### 7. REST API Server (`api_server.rs`)

#### Order Management Endpoints
```
POST /api/v1/orders          # Submit new order
GET  /api/v1/orders/:id      # Get order status  
DELETE /api/v1/orders/:id    # Cancel order
GET  /api/v1/orders          # List orders with filtering
```

#### Position Management Endpoints
```
GET /api/v1/positions           # Get all positions
GET /api/v1/positions/:symbol   # Get specific position
GET /api/v1/positions/summary   # Portfolio summary
```

#### Account Information Endpoints
```
GET /api/v1/account       # Account details and balances
GET /api/v1/account/pnl   # Current P&L breakdown
```

#### System Control Endpoints
```
GET  /api/v1/system/status        # System status
POST /api/v1/system/emergency_stop # Emergency stop
POST /api/v1/system/resume        # Resume trading
GET  /api/v1/system/health        # Health check
```

#### Risk Management Endpoints
```
POST /api/v1/risk/validate  # Validate order against risk rules
GET  /api/v1/risk/limits    # Current risk limits
```

### 8. Execution Engine (`execution_engine.rs`)

#### Neural Signal Processing
- **Signal Validation**: Confidence threshold filtering (60% minimum)
- **Position Sizing**: Dynamic sizing based on signal strength
- **Order Generation**: Automatic order creation from neural predictions

#### Background Services
- **Order Monitoring**: Real-time order status tracking (1-second intervals)
- **Position Updates**: Market price updates (5-second intervals)
- **Account Sync**: Broker synchronization (30-second intervals)

#### Emergency Features
- **Emergency Liquidation**: Automatic position closure on critical events
- **Connection Monitoring**: Broker connectivity health checks

## Configuration

### Default Settings
```rust
ActionLayerConfig {
    broker: BrokerConfig {
        name: "alpaca",
        paper_trading: true,
        base_url: "https://paper-api.alpaca.markets",
    },
    risk: RiskLimits {
        max_position_size: 0.05,        // 5%
        max_daily_loss: 0.02,           // 2%
        max_portfolio_risk: 0.10,       // 10%
        max_drawdown: 0.15,             // 15%
        stop_loss_percentage: 0.02,     // 2%
    },
    paper_trading: true,
    api_port: 8080,
    websocket_port: 8081,
    audit_log_path: "./logs/trading_audit.log",
}
```

## Paper Trading Mode

### Features
- **$100,000 Virtual Capital**: Realistic starting portfolio
- **Simulated Market Data**: Dynamic price movements with spreads
- **Commission Modeling**: Realistic trading costs ($0.005/share, $1-$10 range)
- **Position Tracking**: Full P&L calculations
- **Order Fills**: Realistic execution with market impact

### Safety Controls
- **Manual Order Blocking**: Prevents manual trading in paper mode
- **Risk Validation**: Full risk checks applied to virtual trades
- **Emergency Controls**: All emergency features functional

## Integration with Neural Network

### Signal Processing
1. **Receive Neural Prediction**: Confidence score and direction
2. **Validate Signal Strength**: Minimum 60% confidence threshold
3. **Calculate Position Size**: Kelly criterion with risk adjustment
4. **Risk Validation**: Full risk management checks
5. **Order Submission**: Automatic order generation and submission
6. **Execution Monitoring**: Real-time fill tracking and position updates

### Example Neural Signal Flow
```rust
TradingSignal {
    symbol: "AAPL",
    action: SignalAction::Buy,
    confidence: 0.75,
    target_price: Some(175.50),
    order_type: OrderType::Limit,
    reasoning: "Strong upward momentum detected",
    timestamp: Utc::now(),
}
```

## Risk Management Framework

### Multi-layered Protection
1. **Pre-trade Validation**: Order validation before broker submission
2. **Position Limits**: Real-time position size monitoring
3. **Daily Loss Limits**: Automatic trading halt on loss thresholds
4. **Emergency Controls**: Manual and automatic emergency stops
5. **Correlation Controls**: Sector and asset class exposure limits

### Risk Calculation Example
```rust
// Position risk as % of portfolio
let position_risk = (quantity * price) / portfolio_value;

// Maximum allowed: 5% per position
if position_risk > 0.05 {
    return Err(ActionLayerError::RiskLimitExceeded(...));
}
```

## Testing Strategy

### Paper Trading Validation
1. **Neural Integration Testing**: End-to-end signal processing
2. **Risk System Testing**: Limit validation and enforcement
3. **Emergency Control Testing**: Stop/resume functionality
4. **API Testing**: All REST endpoints with various scenarios
5. **Position Tracking**: P&L accuracy validation

### Production Readiness Checks
- [ ] Broker API credentials configured
- [ ] Risk limits appropriate for account size
- [ ] Emergency contacts and procedures established
- [ ] Audit logging configured and tested
- [ ] Monitoring and alerting systems active

## Deployment Architecture

### MVP Deployment
- **Single Instance**: All components in one process
- **SQLite Storage**: Local audit log storage
- **File-based Config**: Simple configuration management
- **Process Monitoring**: Basic health checks

### Production Considerations
- **High Availability**: Multi-instance deployment
- **Database**: PostgreSQL for audit logs
- **Message Queues**: Redis for real-time updates
- **Monitoring**: Prometheus metrics and Grafana dashboards
- **Logging**: Centralized log aggregation

## API Usage Examples

### Submit Buy Order
```bash
curl -X POST http://localhost:8080/api/v1/orders \
  -H "Content-Type: application/json" \
  -d '{
    "symbol": "AAPL",
    "side": "buy",
    "quantity": 100,
    "order_type": "limit",
    "price": 175.50
  }'
```

### Check Positions
```bash
curl http://localhost:8080/api/v1/positions
```

### Emergency Stop
```bash
curl -X POST http://localhost:8080/api/v1/system/emergency_stop \
  -H "Content-Type: application/json" \
  -d '{
    "reason": "Market volatility exceeds acceptable levels"
  }'
```

## Performance Requirements

### Latency Targets
- **Order Submission**: < 100ms end-to-end
- **Risk Validation**: < 10ms per check
- **Position Updates**: < 50ms market data to P&L update
- **Emergency Stop**: < 1 second to halt all activity

### Throughput Requirements
- **Orders per Second**: 10-50 orders/second peak
- **Position Updates**: 1000 updates/second
- **API Requests**: 100 requests/second
- **Concurrent Users**: 10-20 simultaneous API users

## Security Considerations

### API Security
- **HTTPS Only**: All API communication encrypted
- **API Key Authentication**: Broker credentials protection
- **Rate Limiting**: DoS protection on all endpoints
- **Input Validation**: Complete request validation

### Trading Security
- **Paper Mode Default**: Safe-by-default configuration
- **Emergency Stops**: Multiple layers of trading halts
- **Audit Trail**: Complete transaction logging
- **Access Controls**: Role-based API access

## Monitoring and Alerts

### Key Metrics
- **Order Fill Rate**: Percentage of orders successfully filled
- **Average Fill Time**: Time from submission to execution
- **Risk Violations**: Count of risk limit breaches
- **Emergency Stops**: Frequency and causes
- **P&L Tracking**: Real-time performance monitoring

### Alert Conditions
- **Emergency Stop Triggered**: Immediate notification
- **Risk Limit Approached**: 90% of daily loss limit
- **Broker Connectivity Lost**: Connection failures
- **Unusual Activity**: Abnormal trading patterns
- **System Errors**: Any critical system failures

This Action Layer MVP provides a robust foundation for neural trading while maintaining the simplicity needed for rapid development and deployment.