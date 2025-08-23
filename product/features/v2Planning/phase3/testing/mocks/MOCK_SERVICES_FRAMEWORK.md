# Neural Trader V2 - Mock Services Framework

## Overview

Comprehensive mocking framework for all external dependencies, enabling isolated testing and reliable test execution without external service dependencies.

## Mock Service Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Mock Service Registry                    │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌────────┐ │
│  │   Market    │ │  Database   │ │    Cache    │ │  Auth  │ │
│  │    Data     │ │   Service   │ │   Service   │ │Service │ │
│  │   Service   │ │             │ │             │ │        │ │
│  └─────────────┘ └─────────────┘ └─────────────┘ └────────┘ │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌────────┐ │
│  │ Notification│ │   Config    │ │   Metrics   │ │  Time  │ │
│  │   Service   │ │   Store     │ │   Service   │ │Service │ │
│  │             │ │             │ │             │ │        │ │
│  └─────────────┘ └─────────────┘ └─────────────┘ └────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## 1. Mock Service Registry

### Core Registry Implementation
```typescript
// tests/mocks/service-registry.ts
interface MockService {
  name: string;
  start(): Promise<void>;
  stop(): Promise<void>;
  reset(): void;
  isRunning(): boolean;
}

export class MockServiceRegistry {
  private services: Map<string, MockService> = new Map();
  private isInitialized = false;

  register(service: MockService): void {
    this.services.set(service.name, service);
  }

  async startAll(): Promise<void> {
    if (this.isInitialized) return;

    const startPromises = Array.from(this.services.values())
      .map(service => service.start());
    
    await Promise.all(startPromises);
    this.isInitialized = true;
  }

  async stopAll(): Promise<void> {
    const stopPromises = Array.from(this.services.values())
      .map(service => service.stop());
    
    await Promise.all(stopPromises);
    this.isInitialized = false;
  }

  resetAll(): void {
    this.services.forEach(service => service.reset());
  }

  getService<T extends MockService>(name: string): T {
    const service = this.services.get(name);
    if (!service) {
      throw new Error(`Mock service '${name}' not found`);
    }
    return service as T;
  }
}
```

## 2. Market Data Service Mock

### WebSocket Market Data Mock
```typescript
// tests/mocks/market-data-service.ts
import { EventEmitter } from 'events';
import WebSocket from 'ws';

export interface MarketDataPoint {
  symbol: string;
  price: number;
  volume: number;
  timestamp: Date;
  bid: number;
  ask: number;
}

export class MockMarketDataService extends EventEmitter implements MockService {
  name = 'market-data-service';
  private server?: WebSocket.Server;
  private connections: Set<WebSocket> = new Set();
  private dataGenerators: Map<string, NodeJS.Timeout> = new Map();
  private isRunning = false;

  async start(): Promise<void> {
    this.server = new WebSocket.Server({ port: 8082 });
    
    this.server.on('connection', (ws) => {
      this.connections.add(ws);
      
      ws.on('message', (message) => {
        const request = JSON.parse(message.toString());
        this.handleSubscription(request, ws);
      });

      ws.on('close', () => {
        this.connections.delete(ws);
      });
    });

    this.isRunning = true;
  }

  async stop(): Promise<void> {
    this.dataGenerators.forEach(timer => clearInterval(timer));
    this.dataGenerators.clear();
    
    this.connections.forEach(ws => ws.close());
    this.connections.clear();

    if (this.server) {
      this.server.close();
    }
    
    this.isRunning = false;
  }

  reset(): void {
    this.dataGenerators.forEach(timer => clearInterval(timer));
    this.dataGenerators.clear();
  }

  isRunning(): boolean {
    return this.isRunning;
  }

  private handleSubscription(request: any, ws: WebSocket): void {
    const { action, symbol } = request;
    
    if (action === 'subscribe') {
      this.startDataGeneration(symbol, ws);
    } else if (action === 'unsubscribe') {
      this.stopDataGeneration(symbol);
    }
  }

  private startDataGeneration(symbol: string, ws: WebSocket): void {
    if (this.dataGenerators.has(symbol)) return;

    const generator = setInterval(() => {
      const data = this.generateMarketData(symbol);
      
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify(data));
      }
    }, 100); // 10 FPS

    this.dataGenerators.set(symbol, generator);
  }

  private stopDataGeneration(symbol: string): void {
    const generator = this.dataGenerators.get(symbol);
    if (generator) {
      clearInterval(generator);
      this.dataGenerators.delete(symbol);
    }
  }

  private generateMarketData(symbol: string): MarketDataPoint {
    const basePrice = symbol === 'BTCUSD' ? 50000 : 3000;
    const volatility = 0.001; // 0.1% volatility
    
    const priceChange = (Math.random() - 0.5) * basePrice * volatility;
    const price = basePrice + priceChange;
    const spread = price * 0.0001; // 0.01% spread
    
    return {
      symbol,
      price,
      volume: Math.random() * 100,
      timestamp: new Date(),
      bid: price - spread / 2,
      ask: price + spread / 2,
    };
  }

  // Test helper methods
  injectMarketData(symbol: string, data: Partial<MarketDataPoint>): void {
    const fullData = {
      symbol,
      price: 50000,
      volume: 100,
      timestamp: new Date(),
      bid: 49990,
      ask: 50010,
      ...data,
    };

    this.connections.forEach(ws => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify(fullData));
      }
    });
  }

  simulateOutage(duration: number): void {
    setTimeout(() => {
      this.connections.forEach(ws => ws.close(1006, 'Network error'));
    }, 100);

    setTimeout(() => {
      // Simulate reconnection after outage
      this.emit('reconnected');
    }, duration);
  }
}
```

## 3. Database Service Mock

### In-Memory Database Mock
```typescript
// tests/mocks/database-service.ts
export class MockDatabaseService implements MockService {
  name = 'database-service';
  private data: Map<string, Map<string, any>> = new Map();
  private transactions: Set<string> = new Set();

  async start(): Promise<void> {
    this.initializeTables();
  }

  async stop(): Promise<void> {
    this.data.clear();
  }

  reset(): void {
    this.data.clear();
    this.initializeTables();
    this.transactions.clear();
  }

  isRunning(): boolean {
    return this.data.size > 0;
  }

  private initializeTables(): void {
    this.data.set('trades', new Map());
    this.data.set('positions', new Map());
    this.data.set('market_data', new Map());
    this.data.set('symbols', new Map());
  }

  // Mock database operations
  async query(sql: string, params: any[] = []): Promise<any[]> {
    // Simple SQL parsing for common operations
    if (sql.includes('INSERT INTO')) {
      return this.handleInsert(sql, params);
    } else if (sql.includes('SELECT')) {
      return this.handleSelect(sql, params);
    } else if (sql.includes('UPDATE')) {
      return this.handleUpdate(sql, params);
    } else if (sql.includes('DELETE')) {
      return this.handleDelete(sql, params);
    }
    
    throw new Error(`Unsupported SQL operation: ${sql}`);
  }

  async transaction<T>(callback: () => Promise<T>): Promise<T> {
    const transactionId = Math.random().toString(36);
    this.transactions.add(transactionId);
    
    try {
      const result = await callback();
      this.transactions.delete(transactionId);
      return result;
    } catch (error) {
      this.transactions.delete(transactionId);
      throw error;
    }
  }

  private handleInsert(sql: string, params: any[]): any[] {
    const tableMatch = sql.match(/INSERT INTO (\w+)/);
    const tableName = tableMatch?.[1];
    
    if (!tableName || !this.data.has(tableName)) {
      throw new Error(`Table ${tableName} not found`);
    }

    const table = this.data.get(tableName)!;
    const id = params[0] || Math.random().toString(36);
    
    table.set(id, { id, ...params.slice(1) });
    
    return [{ id }];
  }

  private handleSelect(sql: string, params: any[]): any[] {
    const tableMatch = sql.match(/FROM (\w+)/);
    const tableName = tableMatch?.[1];
    
    if (!tableName || !this.data.has(tableName)) {
      throw new Error(`Table ${tableName} not found`);
    }

    const table = this.data.get(tableName)!;
    return Array.from(table.values());
  }

  private handleUpdate(sql: string, params: any[]): any[] {
    // Simplified update implementation
    return [];
  }

  private handleDelete(sql: string, params: any[]): any[] {
    // Simplified delete implementation
    return [];
  }

  // Test helper methods
  seedData(tableName: string, data: Record<string, any>[]): void {
    const table = this.data.get(tableName);
    if (!table) {
      throw new Error(`Table ${tableName} not found`);
    }

    data.forEach(row => {
      table.set(row.id, row);
    });
  }

  getTableData(tableName: string): any[] {
    const table = this.data.get(tableName);
    if (!table) {
      throw new Error(`Table ${tableName} not found`);
    }

    return Array.from(table.values());
  }

  simulateConnectionError(): void {
    throw new Error('Database connection lost');
  }

  simulateSlowQuery(delay: number): void {
    const originalQuery = this.query;
    this.query = async (sql: string, params: any[] = []) => {
      await new Promise(resolve => setTimeout(resolve, delay));
      return originalQuery.call(this, sql, params);
    };
  }
}
```

## 4. Cache Service Mock

### Redis Cache Mock
```typescript
// tests/mocks/cache-service.ts
export class MockCacheService implements MockService {
  name = 'cache-service';
  private cache: Map<string, { value: any; ttl?: number; setAt: number }> = new Map();
  private pubsub: Map<string, Set<Function>> = new Map();

  async start(): Promise<void> {
    // Start TTL cleanup interval
    setInterval(() => this.cleanupExpired(), 1000);
  }

  async stop(): Promise<void> {
    this.cache.clear();
    this.pubsub.clear();
  }

  reset(): void {
    this.cache.clear();
    this.pubsub.clear();
  }

  isRunning(): boolean {
    return true;
  }

  // Redis-like operations
  async get(key: string): Promise<string | null> {
    const entry = this.cache.get(key);
    if (!entry) return null;
    
    if (entry.ttl && Date.now() - entry.setAt > entry.ttl * 1000) {
      this.cache.delete(key);
      return null;
    }
    
    return JSON.stringify(entry.value);
  }

  async set(key: string, value: any, ttl?: number): Promise<void> {
    this.cache.set(key, {
      value: JSON.parse(value),
      ttl,
      setAt: Date.now(),
    });
  }

  async del(key: string): Promise<number> {
    return this.cache.delete(key) ? 1 : 0;
  }

  async exists(key: string): Promise<boolean> {
    return this.cache.has(key);
  }

  async keys(pattern: string): Promise<string[]> {
    const regex = new RegExp(pattern.replace('*', '.*'));
    return Array.from(this.cache.keys()).filter(key => regex.test(key));
  }

  async publish(channel: string, message: string): Promise<number> {
    const subscribers = this.pubsub.get(channel);
    if (subscribers) {
      subscribers.forEach(callback => callback(message));
      return subscribers.size;
    }
    return 0;
  }

  async subscribe(channel: string, callback: Function): Promise<void> {
    if (!this.pubsub.has(channel)) {
      this.pubsub.set(channel, new Set());
    }
    this.pubsub.get(channel)!.add(callback);
  }

  async unsubscribe(channel: string, callback: Function): Promise<void> {
    const subscribers = this.pubsub.get(channel);
    if (subscribers) {
      subscribers.delete(callback);
    }
  }

  private cleanupExpired(): void {
    const now = Date.now();
    
    for (const [key, entry] of this.cache.entries()) {
      if (entry.ttl && now - entry.setAt > entry.ttl * 1000) {
        this.cache.delete(key);
      }
    }
  }

  // Test helper methods
  simulateEviction(): void {
    // Simulate Redis eviction policy
    if (this.cache.size > 100) {
      const keys = Array.from(this.cache.keys());
      const evictKey = keys[Math.floor(Math.random() * keys.length)];
      this.cache.delete(evictKey);
    }
  }

  simulateConnectionLoss(duration: number): void {
    const originalGet = this.get;
    const originalSet = this.set;
    
    this.get = async () => { throw new Error('Connection lost'); };
    this.set = async () => { throw new Error('Connection lost'); };
    
    setTimeout(() => {
      this.get = originalGet;
      this.set = originalSet;
    }, duration);
  }

  getStats(): { keys: number; memory: number } {
    return {
      keys: this.cache.size,
      memory: JSON.stringify(Array.from(this.cache.values())).length,
    };
  }
}
```

## 5. HTTP Service Mocks (MSW)

### Market Data API Mock
```typescript
// tests/mocks/http/market-data-api.ts
import { rest } from 'msw';

export const marketDataHandlers = [
  // Get current price
  rest.get('/api/market/:symbol/price', (req, res, ctx) => {
    const { symbol } = req.params;
    
    const basePrice = symbol === 'BTCUSD' ? 50000 : 3000;
    const price = basePrice + (Math.random() - 0.5) * basePrice * 0.001;
    
    return res(
      ctx.json({
        symbol,
        price,
        timestamp: new Date().toISOString(),
      })
    );
  }),

  // Get historical data
  rest.get('/api/market/:symbol/history', (req, res, ctx) => {
    const { symbol } = req.params;
    const limit = Number(req.url.searchParams.get('limit')) || 100;
    
    const data = Array.from({ length: limit }, (_, i) => ({
      symbol,
      price: 50000 + Math.sin(i / 10) * 1000,
      volume: Math.random() * 100,
      timestamp: new Date(Date.now() - i * 60000).toISOString(),
    }));
    
    return res(ctx.json(data));
  }),

  // WebSocket endpoint simulation
  rest.get('/ws/market/:symbol', (req, res, ctx) => {
    return res(
      ctx.status(101),
      ctx.set('Upgrade', 'websocket'),
      ctx.set('Connection', 'Upgrade'),
    );
  }),
];
```

## 6. Authentication Service Mock

### JWT Authentication Mock
```typescript
// tests/mocks/auth-service.ts
import jwt from 'jsonwebtoken';

export class MockAuthService implements MockService {
  name = 'auth-service';
  private users: Map<string, any> = new Map();
  private sessions: Set<string> = new Set();
  private readonly secret = 'test-secret';

  async start(): Promise<void> {
    this.seedTestUsers();
  }

  async stop(): Promise<void> {
    this.users.clear();
    this.sessions.clear();
  }

  reset(): void {
    this.users.clear();
    this.sessions.clear();
    this.seedTestUsers();
  }

  isRunning(): boolean {
    return true;
  }

  private seedTestUsers(): void {
    this.users.set('testuser', {
      id: 'user-1',
      username: 'testuser',
      email: 'test@example.com',
      passwordHash: 'hashed-password',
      roles: ['trader'],
    });
  }

  async authenticate(username: string, password: string): Promise<string | null> {
    const user = this.users.get(username);
    if (!user || password !== 'testpass') {
      return null;
    }

    const token = jwt.sign(
      { userId: user.id, username: user.username, roles: user.roles },
      this.secret,
      { expiresIn: '1h' }
    );

    this.sessions.add(token);
    return token;
  }

  async validateToken(token: string): Promise<any | null> {
    if (!this.sessions.has(token)) {
      return null;
    }

    try {
      return jwt.verify(token, this.secret);
    } catch {
      this.sessions.delete(token);
      return null;
    }
  }

  async logout(token: string): Promise<void> {
    this.sessions.delete(token);
  }

  // Test helper methods
  createTestUser(userData: any): string {
    const id = `user-${Date.now()}`;
    this.users.set(userData.username, { id, ...userData });
    return id;
  }

  revokeAllSessions(): void {
    this.sessions.clear();
  }

  simulateTokenExpiry(token: string): void {
    this.sessions.delete(token);
  }
}
```

## 7. Time Service Mock

### Controllable Time Mock
```typescript
// tests/mocks/time-service.ts
export class MockTimeService implements MockService {
  name = 'time-service';
  private currentTime: Date = new Date();
  private timers: Map<string, NodeJS.Timeout> = new Map();
  private intervals: Map<string, NodeJS.Timeout> = new Map();

  async start(): Promise<void> {
    // Override global Date
    (global as any).Date = class extends Date {
      constructor(...args: any[]) {
        if (args.length === 0) {
          super(MockTimeService.instance.currentTime);
        } else {
          super(...args);
        }
      }
      
      static now(): number {
        return MockTimeService.instance.currentTime.getTime();
      }
    };
  }

  async stop(): Promise<void> {
    this.timers.forEach(timer => clearTimeout(timer));
    this.intervals.forEach(interval => clearInterval(interval));
    
    // Restore original Date
    (global as any).Date = Date;
  }

  reset(): void {
    this.currentTime = new Date();
    this.timers.clear();
    this.intervals.clear();
  }

  isRunning(): boolean {
    return true;
  }

  // Time manipulation methods
  setTime(date: Date): void {
    this.currentTime = new Date(date);
  }

  advanceTime(milliseconds: number): void {
    this.currentTime = new Date(this.currentTime.getTime() + milliseconds);
  }

  advanceMinutes(minutes: number): void {
    this.advanceTime(minutes * 60 * 1000);
  }

  advanceHours(hours: number): void {
    this.advanceTime(hours * 60 * 60 * 1000);
  }

  advanceDays(days: number): void {
    this.advanceTime(days * 24 * 60 * 60 * 1000);
  }

  // Timer control
  runAllTimers(): void {
    // Simulate all pending timers
    this.timers.forEach(timer => {
      // Timer logic would be more complex in real implementation
    });
  }

  private static instance: MockTimeService;
  
  static getInstance(): MockTimeService {
    if (!MockTimeService.instance) {
      MockTimeService.instance = new MockTimeService();
    }
    return MockTimeService.instance;
  }
}
```

## 8. Mock Service Integration Example

### Test Integration
```typescript
// tests/integration/trading-service.test.ts
describe('Trading Service Integration', () => {
  let mockRegistry: MockServiceRegistry;
  let marketDataService: MockMarketDataService;
  let databaseService: MockDatabaseService;
  let cacheService: MockCacheService;
  let timeService: MockTimeService;

  beforeAll(async () => {
    mockRegistry = new MockServiceRegistry();
    
    marketDataService = new MockMarketDataService();
    databaseService = new MockDatabaseService();
    cacheService = new MockCacheService();
    timeService = MockTimeService.getInstance();

    mockRegistry.register(marketDataService);
    mockRegistry.register(databaseService);
    mockRegistry.register(cacheService);
    mockRegistry.register(timeService);

    await mockRegistry.startAll();
  });

  beforeEach(() => {
    mockRegistry.resetAll();
  });

  afterAll(async () => {
    await mockRegistry.stopAll();
  });

  it('should execute trade with mocked market data', async () => {
    // Inject specific market data
    marketDataService.injectMarketData('BTCUSD', {
      price: 50000,
      bid: 49990,
      ask: 50010,
    });

    // Set specific time
    timeService.setTime(new Date('2024-01-01T12:00:00Z'));

    // Execute trade
    const tradeRequest = {
      symbol: 'BTCUSD',
      side: 'buy',
      quantity: 0.1,
    };

    const result = await tradingService.executeTrade(tradeRequest);

    // Verify trade was stored in database
    const trades = databaseService.getTableData('trades');
    expect(trades).toHaveLength(1);
    expect(trades[0].symbol).toBe('BTCUSD');
  });
});
```

This comprehensive mock framework enables reliable, fast, and isolated testing of all system components without external dependencies.