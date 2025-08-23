# Neural Trader V2 - Test Infrastructure Setup

## Overview

Comprehensive testing infrastructure for greenfield Neural Trader V2 build, supporting TDD methodology with automated quality gates and continuous validation.

## Core Infrastructure Components

### 1. Test Framework Configuration

#### Jest Configuration (`jest.config.js`)
```javascript
module.exports = {
  preset: 'ts-jest',
  testEnvironment: 'node',
  roots: ['<rootDir>/src', '<rootDir>/tests'],
  testMatch: [
    '**/__tests__/**/*.test.(ts|js)',
    '**/?(*.)+(spec|test).(ts|js)'
  ],
  collectCoverageFrom: [
    'src/**/*.{ts,js}',
    '!src/**/*.d.ts',
    '!src/**/*.interface.ts',
    '!src/**/index.ts'
  ],
  coverageThreshold: {
    global: {
      branches: 95,
      functions: 95,
      lines: 95,
      statements: 95
    }
  },
  setupFilesAfterEnv: ['<rootDir>/tests/setup.ts'],
  testTimeout: 30000,
  maxWorkers: '50%',
  cache: true,
  verbose: true
};
```

#### TypeScript Test Configuration (`tsconfig.test.json`)
```json
{
  "extends": "./tsconfig.json",
  "compilerOptions": {
    "types": ["jest", "node"],
    "esModuleInterop": true,
    "allowSyntheticDefaultImports": true
  },
  "include": [
    "src/**/*",
    "tests/**/*"
  ]
}
```

### 2. Docker Test Infrastructure

#### Test Containers (`docker-compose.test.yml`)
```yaml
version: '3.8'
services:
  postgres-test:
    image: postgres:15
    environment:
      POSTGRES_DB: neural_trader_test
      POSTGRES_USER: test_user
      POSTGRES_PASSWORD: test_pass
    ports:
      - "5433:5432"
    volumes:
      - ./tests/fixtures/postgres:/docker-entrypoint-initdb.d

  redis-test:
    image: redis:7
    ports:
      - "6380:6379"
    command: redis-server --maxmemory 128mb

  mock-market-api:
    image: wiremock/wiremock:2.35.0
    ports:
      - "8081:8080"
    volumes:
      - ./tests/mocks/wiremock:/home/wiremock

  test-runner:
    build:
      context: .
      dockerfile: tests/Dockerfile.test-runner
    depends_on:
      - postgres-test
      - redis-test
      - mock-market-api
    environment:
      NODE_ENV: test
      DATABASE_URL: postgres://test_user:test_pass@postgres-test:5432/neural_trader_test
      REDIS_URL: redis://redis-test:6379
    volumes:
      - .:/app
      - /app/node_modules
```

### 3. Test Database Setup

#### Database Migration Test Setup
```typescript
// tests/setup/database.ts
import { Pool } from 'pg';
import { migrate } from 'postgres-migrations';

export class TestDatabase {
  private pool: Pool;
  private static instance: TestDatabase;

  constructor() {
    this.pool = new Pool({
      connectionString: process.env.DATABASE_URL,
      max: 5,
      idleTimeoutMillis: 30000,
    });
  }

  static getInstance(): TestDatabase {
    if (!TestDatabase.instance) {
      TestDatabase.instance = new TestDatabase();
    }
    return TestDatabase.instance;
  }

  async setupTestDatabase(): Promise<void> {
    // Run migrations
    await migrate({
      client: this.pool,
      migrationsDirectory: './migrations',
      createSchema: true,
    });

    // Seed test data
    await this.seedTestData();
  }

  async cleanDatabase(): Promise<void> {
    const client = await this.pool.connect();
    try {
      await client.query('TRUNCATE TABLE trades, positions, market_data CASCADE');
    } finally {
      client.release();
    }
  }

  private async seedTestData(): Promise<void> {
    // Insert minimal test data
    const client = await this.pool.connect();
    try {
      await client.query(`
        INSERT INTO symbols (symbol, name, exchange) 
        VALUES 
          ('BTCUSD', 'Bitcoin USD', 'BINANCE'),
          ('ETHUSD', 'Ethereum USD', 'BINANCE')
      `);
    } finally {
      client.release();
    }
  }

  async close(): Promise<void> {
    await this.pool.end();
  }
}
```

### 4. Test Setup and Teardown

#### Global Test Setup (`tests/setup.ts`)
```typescript
import { TestDatabase } from './setup/database';
import { TestRedis } from './setup/redis';
import { MockServiceRegistry } from './mocks/service-registry';

let testDatabase: TestDatabase;
let testRedis: TestRedis;
let mockServices: MockServiceRegistry;

beforeAll(async () => {
  // Initialize test infrastructure
  testDatabase = TestDatabase.getInstance();
  await testDatabase.setupTestDatabase();

  testRedis = TestRedis.getInstance();
  await testRedis.connect();

  mockServices = new MockServiceRegistry();
  await mockServices.startAll();

  // Set test timeouts
  jest.setTimeout(30000);
}, 60000);

beforeEach(async () => {
  // Clean state before each test
  await testDatabase.cleanDatabase();
  await testRedis.flushall();
  mockServices.resetAll();
});

afterAll(async () => {
  // Cleanup after all tests
  await testDatabase.close();
  await testRedis.close();
  await mockServices.stopAll();
}, 30000);
```

### 5. Test Utilities and Helpers

#### Test Data Factory
```typescript
// tests/factories/index.ts
import { Factory } from 'fishery';
import { Trade, Position, MarketData } from '../types';

export const TradeFactory = Factory.define<Trade>(({ sequence }) => ({
  id: `trade-${sequence}`,
  symbol: 'BTCUSD',
  side: 'buy',
  quantity: 0.1,
  price: 50000,
  timestamp: new Date(),
  status: 'filled',
}));

export const PositionFactory = Factory.define<Position>(({ sequence }) => ({
  id: `position-${sequence}`,
  symbol: 'BTCUSD',
  quantity: 1.0,
  entryPrice: 50000,
  currentPrice: 51000,
  unrealizedPnl: 1000,
  realizedPnl: 0,
  timestamp: new Date(),
}));

export const MarketDataFactory = Factory.define<MarketData>(() => ({
  symbol: 'BTCUSD',
  price: 50000,
  volume: 100,
  timestamp: new Date(),
  bid: 49990,
  ask: 50010,
}));
```

#### Test Assertion Helpers
```typescript
// tests/helpers/assertions.ts
export const assertTrade = (actual: Trade, expected: Partial<Trade>) => {
  expect(actual.symbol).toBe(expected.symbol);
  expect(actual.side).toBe(expected.side);
  expect(actual.quantity).toBeCloseTo(expected.quantity || 0, 8);
  expect(actual.price).toBeCloseTo(expected.price || 0, 2);
};

export const assertPosition = (actual: Position, expected: Partial<Position>) => {
  expect(actual.symbol).toBe(expected.symbol);
  expect(actual.quantity).toBeCloseTo(expected.quantity || 0, 8);
  expect(actual.entryPrice).toBeCloseTo(expected.entryPrice || 0, 2);
};

export const assertMarketData = (actual: MarketData, expected: Partial<MarketData>) => {
  expect(actual.symbol).toBe(expected.symbol);
  expect(actual.price).toBeCloseTo(expected.price || 0, 2);
  expect(actual.timestamp).toBeInstanceOf(Date);
};
```

### 6. Performance Test Infrastructure

#### Load Testing Setup
```typescript
// tests/performance/load-test-runner.ts
import { check, sleep } from 'k6';
import http from 'k6/http';

export let options = {
  stages: [
    { duration: '2m', target: 100 }, // Ramp up
    { duration: '5m', target: 100 }, // Stay at 100 users
    { duration: '2m', target: 200 }, // Ramp up to 200
    { duration: '5m', target: 200 }, // Stay at 200 users
    { duration: '2m', target: 0 },   // Ramp down
  ],
  thresholds: {
    http_req_duration: ['p(95)<100'], // 95% of requests under 100ms
    http_req_failed: ['rate<0.1'],    // Error rate under 10%
  },
};

export default function () {
  const response = http.get('http://localhost:3000/api/positions');
  
  check(response, {
    'status is 200': (r) => r.status === 200,
    'response time < 100ms': (r) => r.timings.duration < 100,
  });

  sleep(1);
}
```

### 7. CI/CD Pipeline Integration

#### GitHub Actions Test Workflow
```yaml
# .github/workflows/test.yml
name: Test Suite

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: ubuntu-latest
    
    services:
      postgres:
        image: postgres:15
        env:
          POSTGRES_PASSWORD: test_pass
          POSTGRES_USER: test_user
          POSTGRES_DB: neural_trader_test
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
        ports:
          - 5433:5432

      redis:
        image: redis:7
        options: >-
          --health-cmd "redis-cli ping"
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
        ports:
          - 6380:6379

    steps:
      - uses: actions/checkout@v3

      - name: Setup Node.js
        uses: actions/setup-node@v3
        with:
          node-version: '18'
          cache: 'npm'

      - name: Install dependencies
        run: npm ci

      - name: Run unit tests
        run: npm run test:unit
        env:
          NODE_ENV: test

      - name: Run integration tests
        run: npm run test:integration
        env:
          NODE_ENV: test
          DATABASE_URL: postgres://test_user:test_pass@localhost:5433/neural_trader_test
          REDIS_URL: redis://localhost:6380

      - name: Run E2E tests
        run: npm run test:e2e
        env:
          NODE_ENV: test

      - name: Upload coverage reports
        uses: codecov/codecov-action@v3
        with:
          file: ./coverage/lcov.info
```

### 8. Test Reporting and Metrics

#### Coverage Reporting
```typescript
// tests/reporters/coverage-reporter.ts
export class CoverageReporter {
  generateReport(): void {
    const coverage = global.__coverage__;
    
    // Generate HTML report
    // Generate JSON report
    // Generate LCOV report
    // Send metrics to monitoring system
  }

  checkThresholds(): boolean {
    // Validate coverage meets requirements
    return true;
  }
}
```

## Package.json Test Scripts

```json
{
  "scripts": {
    "test": "jest",
    "test:unit": "jest --testPathPattern=unit",
    "test:integration": "jest --testPathPattern=integration",
    "test:e2e": "jest --testPathPattern=e2e",
    "test:coverage": "jest --coverage",
    "test:watch": "jest --watch",
    "test:ci": "jest --ci --coverage --watchAll=false",
    "test:performance": "k6 run tests/performance/load-test.js",
    "test:security": "npm audit && snyk test"
  }
}
```

## Infrastructure Dependencies

```json
{
  "devDependencies": {
    "@types/jest": "^29.5.0",
    "jest": "^29.5.0",
    "ts-jest": "^29.1.0",
    "supertest": "^6.3.0",
    "testcontainers": "^9.1.0",
    "msw": "^1.2.0",
    "k6": "^0.44.0",
    "playwright": "^1.32.0",
    "fishery": "^2.2.0",
    "@faker-js/faker": "^7.6.0"
  }
}
```

This infrastructure provides a robust foundation for TDD development with automated quality gates and comprehensive testing coverage.