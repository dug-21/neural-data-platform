# SPARC API Specifications: Neural Trading Platform

## API Overview

The Neural Trading Platform exposes RESTful APIs for market data ingestion, neural predictions, trading operations, and system monitoring. All APIs follow OpenAPI 3.0 specification.

## Base Configuration

```yaml
openapi: 3.0.0
info:
  title: Neural Trading Platform API
  version: 1.0.0
  description: Autonomous trading platform with neural network predictions
  contact:
    name: Platform Team
    email: platform@neuraltrader.io

servers:
  - url: https://api.neuraltrader.io/v1
    description: Production
  - url: https://staging-api.neuraltrader.io/v1
    description: Staging
  - url: http://localhost:8080/v1
    description: Development

security:
  - bearerAuth: []
  - apiKey: []
```

## Authentication APIs

### POST /auth/login
```yaml
/auth/login:
  post:
    summary: User authentication
    operationId: login
    tags: [Authentication]
    security: []
    requestBody:
      required: true
      content:
        application/json:
          schema:
            type: object
            required: [email, password]
            properties:
              email:
                type: string
                format: email
                example: trader@example.com
              password:
                type: string
                format: password
                minLength: 8
    responses:
      200:
        description: Successful authentication
        content:
          application/json:
            schema:
              type: object
              properties:
                token:
                  type: string
                  description: JWT access token
                refreshToken:
                  type: string
                  description: Refresh token
                expiresIn:
                  type: integer
                  description: Token expiry in seconds
                user:
                  $ref: '#/components/schemas/User'
      401:
        $ref: '#/components/responses/Unauthorized'
```

### POST /auth/refresh
```yaml
/auth/refresh:
  post:
    summary: Refresh access token
    operationId: refreshToken
    tags: [Authentication]
    security: []
    requestBody:
      required: true
      content:
        application/json:
          schema:
            type: object
            required: [refreshToken]
            properties:
              refreshToken:
                type: string
    responses:
      200:
        description: New access token
        content:
          application/json:
            schema:
              type: object
              properties:
                token:
                  type: string
                expiresIn:
                  type: integer
```

## Neural Prediction APIs

### POST /neural/predict
```yaml
/neural/predict:
  post:
    summary: Generate neural network predictions
    operationId: predict
    tags: [Neural]
    requestBody:
      required: true
      content:
        application/json:
          schema:
            type: object
            required: [symbol, horizon]
            properties:
              symbol:
                type: string
                description: Trading symbol
                example: BTC/USDT
              horizon:
                type: integer
                description: Prediction horizon in minutes
                minimum: 1
                maximum: 1440
                example: 60
              features:
                type: object
                description: Additional features for prediction
                additionalProperties: true
              lookbackWindow:
                type: integer
                description: Historical data window in hours
                default: 24
    responses:
      200:
        description: Prediction results
        content:
          application/json:
            schema:
              type: object
              properties:
                predictions:
                  type: array
                  items:
                    $ref: '#/components/schemas/PredictionResult'
                metadata:
                  type: object
                  properties:
                    modelUsed:
                      type: string
                    processingTime:
                      type: number
                    confidence:
                      type: number
```

### POST /neural/predict-ensemble
```yaml
/neural/predict-ensemble:
  post:
    summary: Generate ensemble predictions from multiple models
    operationId: predictEnsemble
    tags: [Neural]
    requestBody:
      required: true
      content:
        application/json:
          schema:
            type: object
            required: [symbol, horizon, models]
            properties:
              symbol:
                type: string
              horizon:
                type: integer
              models:
                type: array
                items:
                  type: string
                  enum: [MLP, LSTM, GRU, TCN, DeepAR, NHITS, Transformer]
              aggregationMethod:
                type: string
                enum: [mean, weighted, median, voting]
                default: weighted
    responses:
      200:
        description: Ensemble predictions
        content:
          application/json:
            schema:
              type: object
              properties:
                ensemble:
                  $ref: '#/components/schemas/PredictionResult'
                individualPredictions:
                  type: object
                  additionalProperties:
                    $ref: '#/components/schemas/PredictionResult'
```

## Trading APIs

### POST /trading/decision
```yaml
/trading/decision:
  post:
    summary: Request autonomous trading decision
    operationId: getTradingDecision
    tags: [Trading]
    requestBody:
      required: true
      content:
        application/json:
          schema:
            type: object
            required: [symbol]
            properties:
              symbol:
                type: string
              currentPosition:
                $ref: '#/components/schemas/Position'
              riskParameters:
                type: object
                properties:
                  maxPositionSize:
                    type: number
                  stopLossPercent:
                    type: number
                  takeProfitPercent:
                    type: number
    responses:
      200:
        description: Trading decision
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/AutonomousDecision'
```

### POST /trading/execute
```yaml
/trading/execute:
  post:
    summary: Execute trading order
    operationId: executeTrade
    tags: [Trading]
    requestBody:
      required: true
      content:
        application/json:
          schema:
            type: object
            required: [action, symbol, size]
            properties:
              action:
                type: string
                enum: [buy, sell]
              symbol:
                type: string
              size:
                type: number
                minimum: 0
              orderType:
                type: string
                enum: [market, limit, stop, stopLimit]
                default: market
              price:
                type: number
                description: Required for limit orders
              stopPrice:
                type: number
                description: Required for stop orders
    responses:
      200:
        description: Order execution result
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/OrderResult'
```

### GET /trading/positions
```yaml
/trading/positions:
  get:
    summary: Get current positions
    operationId: getPositions
    tags: [Trading]
    parameters:
      - name: symbol
        in: query
        schema:
          type: string
        description: Filter by symbol
      - name: status
        in: query
        schema:
          type: string
          enum: [open, closed, all]
        default: open
    responses:
      200:
        description: List of positions
        content:
          application/json:
            schema:
              type: object
              properties:
                positions:
                  type: array
                  items:
                    $ref: '#/components/schemas/Position'
                summary:
                  type: object
                  properties:
                    totalValue:
                      type: number
                    totalPnL:
                      type: number
                    openPositions:
                      type: integer
```

## Market Data APIs

### GET /market/data
```yaml
/market/data:
  get:
    summary: Get historical market data
    operationId: getMarketData
    tags: [Market Data]
    parameters:
      - name: symbol
        in: query
        required: true
        schema:
          type: string
      - name: interval
        in: query
        schema:
          type: string
          enum: [1m, 5m, 15m, 30m, 1h, 4h, 1d]
          default: 1h
      - name: start
        in: query
        schema:
          type: string
          format: date-time
      - name: end
        in: query
        schema:
          type: string
          format: date-time
      - name: limit
        in: query
        schema:
          type: integer
          maximum: 1000
          default: 100
    responses:
      200:
        description: Market data
        content:
          application/json:
            schema:
              type: object
              properties:
                data:
                  type: array
                  items:
                    $ref: '#/components/schemas/MarketData'
                metadata:
                  type: object
                  properties:
                    symbol:
                      type: string
                    interval:
                      type: string
                    count:
                      type: integer
```

### WebSocket /market/stream
```yaml
/market/stream:
  get:
    summary: Stream real-time market data
    operationId: streamMarketData
    tags: [Market Data]
    parameters:
      - name: symbols
        in: query
        required: true
        schema:
          type: array
          items:
            type: string
        style: form
        explode: false
      - name: channels
        in: query
        schema:
          type: array
          items:
            type: string
            enum: [ticker, orderbook, trades, kline]
          default: [ticker]
    responses:
      101:
        description: WebSocket connection established
        headers:
          Upgrade:
            schema:
              type: string
              example: websocket
          Connection:
            schema:
              type: string
              example: Upgrade
```

## Strategy APIs

### GET /strategies
```yaml
/strategies:
  get:
    summary: List available strategies
    operationId: listStrategies
    tags: [Strategies]
    responses:
      200:
        description: Available strategies
        content:
          application/json:
            schema:
              type: object
              properties:
                strategies:
                  type: array
                  items:
                    $ref: '#/components/schemas/Strategy'
```

### POST /strategies/{strategyId}/backtest
```yaml
/strategies/{strategyId}/backtest:
  post:
    summary: Run strategy backtest
    operationId: backtestStrategy
    tags: [Strategies]
    parameters:
      - name: strategyId
        in: path
        required: true
        schema:
          type: string
    requestBody:
      required: true
      content:
        application/json:
          schema:
            type: object
            required: [symbol, startDate, endDate]
            properties:
              symbol:
                type: string
              startDate:
                type: string
                format: date-time
              endDate:
                type: string
                format: date-time
              initialCapital:
                type: number
                default: 10000
              parameters:
                type: object
                additionalProperties: true
    responses:
      200:
        description: Backtest results
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/BacktestResult'
```

## Monitoring APIs

### GET /health
```yaml
/health:
  get:
    summary: System health check
    operationId: healthCheck
    tags: [Monitoring]
    security: []
    responses:
      200:
        description: System is healthy
        content:
          application/json:
            schema:
              type: object
              properties:
                status:
                  type: string
                  enum: [healthy, degraded, unhealthy]
                timestamp:
                  type: string
                  format: date-time
                services:
                  type: object
                  additionalProperties:
                    type: object
                    properties:
                      status:
                        type: string
                      latency:
                        type: number
                      lastCheck:
                        type: string
                        format: date-time
```

### GET /metrics
```yaml
/metrics:
  get:
    summary: Get system metrics
    operationId: getMetrics
    tags: [Monitoring]
    parameters:
      - name: service
        in: query
        schema:
          type: string
          enum: [neural, trading, market, all]
        default: all
      - name: period
        in: query
        schema:
          type: string
          enum: [1h, 6h, 24h, 7d, 30d]
        default: 24h
    responses:
      200:
        description: System metrics
        content:
          application/json:
            schema:
              type: object
              properties:
                metrics:
                  type: object
                  additionalProperties:
                    type: object
                    properties:
                      current:
                        type: number
                      average:
                        type: number
                      min:
                        type: number
                      max:
                        type: number
                      percentiles:
                        type: object
                        properties:
                          p50:
                            type: number
                          p95:
                            type: number
                          p99:
                            type: number
```

## Component Schemas

### User
```yaml
User:
  type: object
  properties:
    id:
      type: string
      format: uuid
    email:
      type: string
      format: email
    name:
      type: string
    roles:
      type: array
      items:
        type: string
        enum: [admin, trader, viewer]
    createdAt:
      type: string
      format: date-time
    lastLogin:
      type: string
      format: date-time
```

### PredictionResult
```yaml
PredictionResult:
  type: object
  properties:
    timestamp:
      type: string
      format: date-time
    value:
      type: number
      description: Predicted value
    confidence:
      type: number
      minimum: 0
      maximum: 1
    intervalLow:
      type: number
      description: Lower confidence interval
    intervalHigh:
      type: number
      description: Upper confidence interval
    modelName:
      type: string
    metadata:
      type: object
      additionalProperties: true
```

### AutonomousDecision
```yaml
AutonomousDecision:
  type: object
  properties:
    timestamp:
      type: string
      format: date-time
    action:
      type: object
      oneOf:
        - type: object
          properties:
            type:
              type: string
              enum: [buy]
            symbol:
              type: string
            size:
              type: number
            stopLoss:
              type: number
            takeProfit:
              type: number
        - type: object
          properties:
            type:
              type: string
              enum: [sell]
            symbol:
              type: string
            size:
              type: number
            reason:
              type: string
        - type: object
          properties:
            type:
              type: string
              enum: [hold]
            reason:
              type: string
    confidence:
      type: number
      minimum: 0
      maximum: 1
    riskAssessment:
      type: object
      properties:
        marketRisk:
          type: number
        positionRisk:
          type: number
        portfolioRisk:
          type: number
        volatilityAdjustedSize:
          type: number
    reasoning:
      type: array
      items:
        type: string
    neuralConsensus:
      type: object
      additionalProperties:
        type: number
```

### Position
```yaml
Position:
  type: object
  properties:
    id:
      type: string
      format: uuid
    symbol:
      type: string
    side:
      type: string
      enum: [long, short]
    size:
      type: number
    entryPrice:
      type: number
    currentPrice:
      type: number
    unrealizedPnL:
      type: number
    realizedPnL:
      type: number
    openedAt:
      type: string
      format: date-time
    closedAt:
      type: string
      format: date-time
    status:
      type: string
      enum: [open, closed, liquidated]
```

### MarketData
```yaml
MarketData:
  type: object
  properties:
    timestamp:
      type: string
      format: date-time
    symbol:
      type: string
    open:
      type: number
    high:
      type: number
    low:
      type: number
    close:
      type: number
    volume:
      type: number
    trades:
      type: integer
    vwap:
      type: number
      description: Volume weighted average price
```

## Error Responses

### 400 Bad Request
```yaml
BadRequest:
  description: Invalid request parameters
  content:
    application/json:
      schema:
        type: object
        properties:
          error:
            type: object
            properties:
              code:
                type: string
                example: INVALID_PARAMETERS
              message:
                type: string
              details:
                type: array
                items:
                  type: object
                  properties:
                    field:
                      type: string
                    issue:
                      type: string
```

### 401 Unauthorized
```yaml
Unauthorized:
  description: Authentication required
  content:
    application/json:
      schema:
        type: object
        properties:
          error:
            type: object
            properties:
              code:
                type: string
                example: UNAUTHORIZED
              message:
                type: string
                example: Invalid or expired token
```

### 429 Rate Limited
```yaml
RateLimited:
  description: Too many requests
  headers:
    X-RateLimit-Limit:
      schema:
        type: integer
    X-RateLimit-Remaining:
      schema:
        type: integer
    X-RateLimit-Reset:
      schema:
        type: integer
  content:
    application/json:
      schema:
        type: object
        properties:
          error:
            type: object
            properties:
              code:
                type: string
                example: RATE_LIMITED
              message:
                type: string
              retryAfter:
                type: integer
```

## Security Schemes

```yaml
components:
  securitySchemes:
    bearerAuth:
      type: http
      scheme: bearer
      bearerFormat: JWT
      description: JWT token obtained from /auth/login
    
    apiKey:
      type: apiKey
      in: header
      name: X-API-Key
      description: API key for service-to-service communication
```

## Rate Limiting

All API endpoints implement rate limiting:

- **Authenticated Users**: 1000 requests/minute
- **Unauthenticated**: 100 requests/minute
- **Neural Predictions**: 100 requests/minute
- **Trading Execution**: 50 requests/minute
- **WebSocket Connections**: 10 concurrent per user

Rate limit headers are included in all responses:
- `X-RateLimit-Limit`: Request limit
- `X-RateLimit-Remaining`: Remaining requests
- `X-RateLimit-Reset`: Reset timestamp

## Versioning

The API uses URL versioning (e.g., `/v1/`). Breaking changes will result in a new API version. Non-breaking changes may be added to existing versions.

## WebSocket Events

### Market Data Events
```json
{
  "type": "ticker",
  "symbol": "BTC/USDT",
  "data": {
    "price": 50000.00,
    "bid": 49995.00,
    "ask": 50005.00,
    "volume24h": 1234567.89,
    "change24h": 2.5
  },
  "timestamp": "2024-01-30T12:00:00Z"
}
```

### Trading Events
```json
{
  "type": "orderUpdate",
  "orderId": "550e8400-e29b-41d4-a716-446655440000",
  "status": "filled",
  "filledSize": 0.1,
  "filledPrice": 50000.00,
  "timestamp": "2024-01-30T12:00:00Z"
}
```

This API specification provides a comprehensive interface for all platform functionality while maintaining security, performance, and usability standards.