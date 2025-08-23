# Neural Trader V2 - Greenfield Architecture

## Overview

This document describes the clean, testable architecture for Neural Trader V2, built from scratch without legacy constraints. Every component is designed for testability, maintainability, and quality.

## Architecture Principles

### Clean Architecture
```
┌──────────────────────────────────────────┐
│            Presentation Layer            │ <- REST/gRPC APIs
├──────────────────────────────────────────┤
│           Application Layer              │ <- Use Cases
├──────────────────────────────────────────┤
│             Domain Layer                 │ <- Business Logic
├──────────────────────────────────────────┤
│          Infrastructure Layer            │ <- External Services
└──────────────────────────────────────────┘

Dependencies: ↓ (inward only)
```

### Hexagonal Architecture
```
        ┌─────────────────┐
        │   Domain Core   │
        └────────┬────────┘
                 │
    ┌────────────┼────────────┐
    │            │            │
┌───▼───┐  ┌────▼────┐  ┌───▼───┐
│ Port  │  │  Port   │  │ Port  │
└───┬───┘  └────┬────┘  └───┬───┘
    │           │            │
┌───▼───┐  ┌────▼────┐  ┌───▼───┐
│Adapter│  │ Adapter │  │Adapter│
└───────┘  └─────────┘  └───────┘
  (DB)      (EventBus)    (API)
```

## Component Architecture

### 1. Domain Layer (Core Business Logic)

#### Trading Domain
```typescript
// Domain Entities
export class TradingSignal {
  constructor(
    public readonly symbol: string,
    public readonly action: TradeAction,
    public readonly confidence: number,
    public readonly timestamp: Date
  ) {
    this.validateConfidence();
  }

  private validateConfidence(): void {
    if (this.confidence < 0 || this.confidence > 1) {
      throw new InvalidConfidenceError(this.confidence);
    }
  }
}

// Domain Services
export interface SignalGenerator {
  generateSignal(marketData: MarketData): Promise<TradingSignal>;
}

// Domain Events
export class SignalGeneratedEvent {
  constructor(
    public readonly signal: TradingSignal,
    public readonly metadata: SignalMetadata
  ) {}
}
```

#### ML Domain
```python
# Domain Models
class PredictionModel:
    def __init__(self, model_id: str, version: str):
        self.model_id = model_id
        self.version = version
        self._validate()
    
    def predict(self, features: Features) -> Prediction:
        """Pure business logic for prediction"""
        pass
    
    def _validate(self):
        if not self.model_id:
            raise InvalidModelError("Model ID required")

# Domain Repositories (Interfaces)
class ModelRepository(ABC):
    @abstractmethod
    async def get_model(self, model_id: str) -> PredictionModel:
        pass
    
    @abstractmethod
    async def save_model(self, model: PredictionModel) -> None:
        pass
```

### 2. Application Layer (Use Cases)

#### Use Case Implementation
```typescript
export class GenerateTradingSignalUseCase {
  constructor(
    private readonly marketDataRepo: MarketDataRepository,
    private readonly mlService: MLPredictionService,
    private readonly signalRepo: SignalRepository,
    private readonly eventBus: EventBus
  ) {}

  async execute(request: SignalRequest): Promise<SignalResponse> {
    // 1. Fetch market data
    const marketData = await this.marketDataRepo.getLatest(request.symbol);
    
    // 2. Generate ML prediction
    const prediction = await this.mlService.predict(marketData);
    
    // 3. Create trading signal
    const signal = new TradingSignal(
      request.symbol,
      this.determineAction(prediction),
      prediction.confidence,
      new Date()
    );
    
    // 4. Persist signal
    await this.signalRepo.save(signal);
    
    // 5. Publish event
    await this.eventBus.publish(new SignalGeneratedEvent(signal));
    
    return new SignalResponse(signal);
  }
}
```

### 3. Infrastructure Layer (External Integrations)

#### Repository Implementations
```typescript
export class PostgresSignalRepository implements SignalRepository {
  constructor(private readonly db: DatabaseConnection) {}

  async save(signal: TradingSignal): Promise<void> {
    await this.db.query(
      `INSERT INTO signals (symbol, action, confidence, timestamp) 
       VALUES ($1, $2, $3, $4)`,
      [signal.symbol, signal.action, signal.confidence, signal.timestamp]
    );
  }

  async findById(id: string): Promise<TradingSignal | null> {
    const result = await this.db.query(
      'SELECT * FROM signals WHERE id = $1',
      [id]
    );
    return result.rows[0] ? this.mapToEntity(result.rows[0]) : null;
  }
}
```

#### External Service Adapters
```python
class AlpacaMarketDataAdapter(MarketDataService):
    def __init__(self, api_key: str, api_secret: str):
        self.client = AlpacaClient(api_key, api_secret)
    
    async def get_quote(self, symbol: str) -> Quote:
        """Adapt external API to domain model"""
        alpaca_quote = await self.client.get_latest_quote(symbol)
        return Quote(
            symbol=symbol,
            bid=alpaca_quote['bid_price'],
            ask=alpaca_quote['ask_price'],
            timestamp=alpaca_quote['timestamp']
        )
```

### 4. Presentation Layer (APIs)

#### REST API
```typescript
@Controller('/api/v1/signals')
export class SignalController {
  constructor(
    private readonly generateSignalUseCase: GenerateTradingSignalUseCase
  ) {}

  @Post('/')
  @UseGuards(AuthGuard)
  async generateSignal(@Body() dto: GenerateSignalDto): Promise<SignalDto> {
    const request = this.mapToRequest(dto);
    const response = await this.generateSignalUseCase.execute(request);
    return this.mapToDto(response);
  }

  private mapToRequest(dto: GenerateSignalDto): SignalRequest {
    // DTO to domain mapping
  }

  private mapToDto(response: SignalResponse): SignalDto {
    // Domain to DTO mapping
  }
}
```

#### gRPC Service
```proto
service TradingService {
  rpc GenerateSignal(SignalRequest) returns (SignalResponse);
  rpc StreamSignals(StreamRequest) returns (stream Signal);
}

message SignalRequest {
  string symbol = 1;
  SignalParameters parameters = 2;
}

message SignalResponse {
  Signal signal = 1;
  SignalMetadata metadata = 2;
}
```

## Service Architecture

### Service Boundaries
```yaml
Trading Service:
  Domain: Trading signals, strategies, risk
  API: gRPC + REST
  Storage: PostgreSQL
  Events: Publishes to EventBus

ML Service:
  Domain: Models, predictions, features
  API: gRPC
  Storage: PostgreSQL + S3
  Events: Consumes from EventBus

Market Data Service:
  Domain: Quotes, bars, fundamentals
  API: WebSocket + gRPC
  Storage: TimescaleDB
  Events: Publishes to EventBus

Order Management Service:
  Domain: Orders, executions, positions
  API: gRPC + REST
  Storage: PostgreSQL
  Events: Bidirectional EventBus
```

### Event-Driven Architecture
```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│Market Data  │────▶│  EventBus   │────▶│ML Service   │
└─────────────┘     └──────┬──────┘     └─────────────┘
                           │
                           ▼
                    ┌─────────────┐
                    │Trading Svc  │
                    └──────┬──────┘
                           │
                           ▼
                    ┌─────────────┐
                    │Order Mgmt   │
                    └─────────────┘
```

## Testing Architecture

### Unit Testing
```typescript
describe('TradingSignal', () => {
  it('should validate confidence between 0 and 1', () => {
    expect(() => new TradingSignal('AAPL', 'BUY', 1.5, new Date()))
      .toThrow(InvalidConfidenceError);
  });

  it('should create valid signal', () => {
    const signal = new TradingSignal('AAPL', 'BUY', 0.85, new Date());
    expect(signal.confidence).toBe(0.85);
  });
});
```

### Integration Testing
```python
@pytest.mark.integration
async def test_signal_generation_flow():
    # Setup test containers
    async with TestContainers() as containers:
        db = await containers.postgres()
        eventbus = await containers.nats()
        
        # Setup services with test dependencies
        repo = PostgresSignalRepository(db)
        bus = NATSEventBus(eventbus)
        use_case = GenerateSignalUseCase(repo, bus)
        
        # Execute test
        request = SignalRequest(symbol="AAPL")
        response = await use_case.execute(request)
        
        # Verify
        assert response.signal.symbol == "AAPL"
        assert await repo.find_by_symbol("AAPL") is not None
        assert await bus.get_published_events() == 1
```

### Contract Testing
```typescript
describe('Trading Service API Contract', () => {
  it('should match consumer expectations', async () => {
    const provider = new PactProvider({
      provider: 'TradingService',
      providerBaseUrl: 'http://localhost:8080',
    });

    await provider.verifyProvider({
      consumerVersionTags: ['production'],
      providerVersionTags: ['main'],
    });
  });
});
```

## Deployment Architecture

### Container Architecture
```dockerfile
# Multi-stage build for optimization
FROM rust:1.70 as builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/trading-service /usr/local/bin/
EXPOSE 8080 50051
CMD ["trading-service"]
```

### Kubernetes Deployment
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: trading-service
spec:
  replicas: 3
  selector:
    matchLabels:
      app: trading-service
  template:
    metadata:
      labels:
        app: trading-service
    spec:
      containers:
      - name: trading-service
        image: neural-trader/trading-service:v2.0.0
        ports:
        - containerPort: 8080
        - containerPort: 50051
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: db-secret
              key: url
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 10
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
```

## Monitoring & Observability

### Metrics
```typescript
// Prometheus metrics
export const metrics = {
  signalsGenerated: new Counter({
    name: 'signals_generated_total',
    help: 'Total number of trading signals generated',
    labelNames: ['symbol', 'action'],
  }),
  
  predictionLatency: new Histogram({
    name: 'prediction_latency_seconds',
    help: 'ML prediction latency',
    buckets: [0.01, 0.05, 0.1, 0.5, 1, 2, 5],
  }),
  
  activePositions: new Gauge({
    name: 'active_positions_count',
    help: 'Number of active trading positions',
  }),
};
```

### Distributed Tracing
```python
from opentelemetry import trace

tracer = trace.get_tracer(__name__)

class TradingService:
    @tracer.start_as_current_span("generate_signal")
    async def generate_signal(self, request: SignalRequest):
        span = trace.get_current_span()
        span.set_attribute("symbol", request.symbol)
        
        with tracer.start_as_current_span("fetch_market_data"):
            market_data = await self.fetch_market_data(request.symbol)
        
        with tracer.start_as_current_span("ml_prediction"):
            prediction = await self.predict(market_data)
        
        return Signal(prediction)
```

### Logging
```typescript
import { Logger } from 'winston';

export class TradingService {
  private readonly logger = new Logger({
    service: 'trading-service',
    level: 'info',
  });

  async generateSignal(request: SignalRequest): Promise<Signal> {
    this.logger.info('Generating signal', {
      symbol: request.symbol,
      timestamp: new Date().toISOString(),
    });

    try {
      const signal = await this.processSignal(request);
      this.logger.info('Signal generated successfully', {
        signal: signal.toJSON(),
      });
      return signal;
    } catch (error) {
      this.logger.error('Failed to generate signal', {
        error: error.message,
        stack: error.stack,
        request: request.toJSON(),
      });
      throw error;
    }
  }
}
```

## Security Architecture

### Authentication & Authorization
```typescript
// JWT-based authentication
export class AuthMiddleware {
  async validateToken(token: string): Promise<User> {
    const decoded = jwt.verify(token, process.env.JWT_SECRET);
    return this.userService.findById(decoded.userId);
  }
}

// Role-based access control
export class AuthorizationGuard {
  canAccess(user: User, resource: Resource, action: Action): boolean {
    return this.rbac.hasPermission(user.role, resource, action);
  }
}
```

### Data Encryption
```python
# Encryption at rest
class EncryptedModelStorage:
    def __init__(self, kms_client):
        self.kms = kms_client
    
    async def save_model(self, model: Model):
        encrypted_data = await self.kms.encrypt(
            model.serialize(),
            key_id="model-encryption-key"
        )
        await self.storage.save(encrypted_data)
```

## Performance Optimization

### Caching Strategy
```typescript
// Multi-level caching
export class CachingService {
  constructor(
    private readonly l1Cache: MemoryCache,  // In-process
    private readonly l2Cache: RedisCache,   // Distributed
  ) {}

  async get<T>(key: string): Promise<T | null> {
    // Check L1 cache
    const l1Result = await this.l1Cache.get(key);
    if (l1Result) return l1Result;

    // Check L2 cache
    const l2Result = await this.l2Cache.get(key);
    if (l2Result) {
      await this.l1Cache.set(key, l2Result);
      return l2Result;
    }

    return null;
  }
}
```

### Connection Pooling
```python
# Database connection pooling
class DatabasePool:
    def __init__(self):
        self.pool = asyncpg.create_pool(
            min_size=10,
            max_size=50,
            max_queries=50000,
            max_inactive_connection_lifetime=300,
        )
```

## Conclusion

This greenfield architecture provides:
- **Clean separation of concerns** with layered architecture
- **Testability** at every level with dependency injection
- **Scalability** through microservices and event-driven design
- **Observability** with comprehensive monitoring
- **Security** with defense in depth
- **Performance** with caching and optimization

Every component is designed from scratch with quality as the primary concern, ensuring a maintainable and reliable system.