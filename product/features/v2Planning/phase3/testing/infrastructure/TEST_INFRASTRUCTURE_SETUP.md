# Neural Trader V2 - Test Infrastructure Setup
## Binary Separation Architecture Edition

## Overview

Comprehensive testing infrastructure for the Neural Trader V2 **binary separation architecture**, supporting TDD methodology with Redis Streams integration testing, binary isolation validation, and automated quality gates.

### Architecture Overview
- **4 Independent Binaries**: config-store, data-ingestion, ruv-FANN, DAA Coordinator
- **Communication Layer**: Redis Streams for all cross-binary messaging
- **Languages**: Rust (3 binaries) + Python (1 binary)
- **Testing Focus**: Binary independence, Redis Streams reliability, cross-binary integration

## Core Infrastructure Components

### 1. Binary-Specific Test Framework Configuration

#### Rust Binaries Test Configuration (`Cargo.toml`)
```toml
[dev-dependencies]
tokio-test = "0.4"
testcontainers = "0.15"
redis = { version = "0.24", features = ["tokio-comp"] }
serde_json = "1.0"
mockall = "0.11"
proptest = "1.0"
tracing-test = "0.2"

[[bin]]
name = "config-store"
path = "src/config-store/main.rs"

[[bin]]
name = "ruv-fann"
path = "src/ruv-fann/main.rs"

[[bin]]
name = "daa-coordinator"
path = "src/daa-coordinator/main.rs"

[features]
default = []
test-utils = []
integration-tests = ["testcontainers"]
```

#### Python Binary Test Configuration (`pytest.ini`)
```ini
[tool:pytest]
testpaths = tests/data-ingestion
python_files = test_*.py *_test.py
python_functions = test_*
addopts = 
    --asyncio-mode=auto
    --cov=data_ingestion
    --cov-report=html:htmlcov
    --cov-report=term-missing
    --cov-fail-under=95
    --redis-url=redis://localhost:6379/15
markers =
    unit: Unit tests
    integration: Integration tests with Redis
    redis_streams: Redis Streams specific tests
    slow: Slow running tests
```

#### Cross-Binary Integration Test Configuration (`docker-compose.integration.yml`)
```yaml
version: '3.8'
services:
  redis-streams:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    command: >
      redis-server 
      --maxmemory 512mb
      --maxmemory-policy allkeys-lru
      --appendonly yes
    volumes:
      - redis_test_data:/data
      
  config-store-test:
    build:
      context: .
      dockerfile: tests/dockerfiles/Dockerfile.config-store
    depends_on:
      - redis-streams
    environment:
      - REDIS_URL=redis://redis-streams:6379
      - RUST_LOG=debug
      - RUST_BACKTRACE=1
    volumes:
      - ./tests/config-store:/app/tests
      
  data-ingestion-test:
    build:
      context: .
      dockerfile: tests/dockerfiles/Dockerfile.data-ingestion
    depends_on:
      - redis-streams
    environment:
      - REDIS_URL=redis://redis-streams:6379
      - PYTHONPATH=/app
      - LOG_LEVEL=DEBUG
    volumes:
      - ./tests/data-ingestion:/app/tests

volumes:
  redis_test_data:
```

### 2. Binary-Specific Docker Test Infrastructure

#### Redis Streams Test Infrastructure (`docker-compose.test.yml`)
```yaml
version: '3.8'
services:
  redis-streams-test:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    command: >
      redis-server
      --maxmemory 256mb
      --maxmemory-policy allkeys-lru
      --save 60 1
      --appendonly yes
      --stream-node-max-bytes 4096
      --stream-node-max-entries 100
    volumes:
      - ./tests/redis/redis.conf:/usr/local/etc/redis/redis.conf
      - redis_streams_data:/data
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 5s
      timeout: 3s
      retries: 5

  config-store-binary-test:
    build:
      context: .
      dockerfile: tests/dockerfiles/Dockerfile.rust-test
      args:
        BINARY_NAME: config-store
    depends_on:
      redis-streams-test:
        condition: service_healthy
    environment:
      - REDIS_URL=redis://redis-streams-test:6379
      - GRPC_PORT=50051
      - RUST_LOG=debug,config_store=trace
    ports:
      - "50051:50051"
    command: ["cargo", "test", "--bin", "config-store", "--", "--test-threads=1"]
    
  data-ingestion-binary-test:
    build:
      context: .
      dockerfile: tests/dockerfiles/Dockerfile.python-test
    depends_on:
      redis-streams-test:
        condition: service_healthy
    environment:
      - REDIS_URL=redis://redis-streams-test:6379
      - PYTHONPATH=/app/data_ingestion
      - PYTEST_CURRENT_TEST=""
    command: ["pytest", "-v", "tests/data-ingestion/", "--redis-url=redis://redis-streams-test:6379"]

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

### 3. Redis Streams Test Setup

#### Redis Streams Test Infrastructure
```rust
// tests/common/redis_test_setup.rs
use redis::{Client, Connection, RedisResult};
use testcontainers::clients::Cli;
use testcontainers::images::redis::Redis;
use tokio::time::{sleep, Duration};

pub struct RedisStreamTestSetup {
    client: Client,
    container: testcontainers::Container<'static, Redis>,
}

impl RedisStreamTestSetup {
    pub async fn new() -> RedisResult<Self> {
        let docker = Cli::default();
        let container = docker.run(Redis::default());
        let port = container.get_host_port_ipv4(6379);
        
        let redis_url = format!("redis://127.0.0.1:{}", port);
        let client = Client::open(redis_url)?;
        
        // Wait for Redis to be ready
        sleep(Duration::from_millis(100)).await;
        
        Ok(Self { client, container })
    }

    pub async fn create_test_streams(&self) -> RedisResult<()> {
        let mut conn = self.client.get_async_connection().await?;
        
        // Create test streams for each binary communication channel
        let streams = [
            "config-updates",
            "market-data", 
            "neural-signals",
            "agent-coordination",
            "system-events"
        ];
        
        for stream in streams {
            // Initialize stream with a dummy message
            redis::cmd("XADD")
                .arg(stream)
                .arg("*")
                .arg("init")
                .arg("test_setup")
                .query_async(&mut conn)
                .await?;
        }
        
        Ok(())
    }

    pub async fn cleanup_streams(&self) -> RedisResult<()> {
        let mut conn = self.client.get_async_connection().await?;
        
        // Delete all test streams
        let streams: Vec<String> = redis::cmd("KEYS")
            .arg("*")
            .query_async(&mut conn)
            .await?;
            
        for stream in streams {
            redis::cmd("DEL")
                .arg(&stream)
                .query_async(&mut conn)
                .await?;
        }
        
        Ok(())
    }
    
    pub fn get_client(&self) -> &Client {
        &self.client
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

#### Python Binary Test Setup (`tests/conftest.py`)
```python
# tests/data-ingestion/conftest.py
import asyncio
import pytest
import redis.asyncio as redis
from testcontainers.redis import RedisContainer
from data_ingestion.stream_processor import StreamProcessor
from data_ingestion.config.settings import get_test_settings

@pytest.fixture(scope="session")
def event_loop():
    """Create an instance of the default event loop for the test session."""
    loop = asyncio.get_event_loop_policy().new_event_loop()
    yield loop
    loop.close()

@pytest.fixture(scope="session")
async def redis_container():
    """Start Redis container for integration tests."""
    with RedisContainer("redis:7-alpine") as container:
        yield container

@pytest.fixture(scope="session")
async def redis_client(redis_container):
    """Create Redis client connected to test container."""
    port = redis_container.get_exposed_port(6379)
    client = redis.Redis(host="localhost", port=port, decode_responses=True)
    
    # Wait for Redis to be ready
    await asyncio.sleep(0.1)
    await client.ping()
    
    yield client
    await client.close()

@pytest.fixture
async def clean_redis_streams(redis_client):
    """Clean Redis streams before each test."""
    # Get all keys
    keys = await redis_client.keys("*")
    if keys:
        await redis_client.delete(*keys)
    
    # Create fresh test streams
    streams = [
        "config-updates",
        "market-data", 
        "neural-signals",
        "agent-coordination",
        "system-events"
    ]
    
    for stream in streams:
        await redis_client.xadd(stream, {"init": "test_setup"})
    
    yield redis_client
    
    # Cleanup after test
    keys = await redis_client.keys("*")
    if keys:
        await redis_client.delete(*keys)

afterAll(async () => {
  // Cleanup after all tests
  await testDatabase.close();
  await testRedis.close();
  await mockServices.stopAll();
}, 30000);
```

### 5. Binary-Specific Test Utilities

#### Redis Streams Test Utilities (Rust)
```rust
// tests/common/stream_test_helpers.rs
use redis::{Client, RedisResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::time::{timeout, Duration};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TestMessage {
    pub id: String,
    pub data: HashMap<String, String>,
    pub timestamp: u64,
}

pub struct StreamTestHelper {
    client: Client,
}

impl StreamTestHelper {
    pub fn new(redis_url: &str) -> RedisResult<Self> {
        let client = Client::open(redis_url)?;
        Ok(Self { client })
    }
    
    pub async fn publish_test_message(
        &self,
        stream: &str,
        message: &TestMessage,
    ) -> RedisResult<String> {
        let mut conn = self.client.get_async_connection().await?;
        
        redis::cmd("XADD")
            .arg(stream)
            .arg("*")
            .arg("data")
            .arg(serde_json::to_string(message).unwrap())
            .query_async(&mut conn)
            .await
    }

    pub async fn consume_messages_with_timeout(
        &self,
        stream: &str,
        group: &str,
        consumer: &str,
        timeout_ms: u64,
    ) -> RedisResult<Vec<TestMessage>> {
        let mut conn = self.client.get_async_connection().await?;
        
        // Create consumer group if it doesn't exist
        let _: Result<String, _> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(stream)
            .arg(group)
            .arg("0")
            .arg("MKSTREAM")
            .query_async(&mut conn)
            .await;
            
        // Read messages with timeout
        let result = timeout(
            Duration::from_millis(timeout_ms),
            redis::cmd("XREADGROUP")
                .arg("GROUP")
                .arg(group)
                .arg(consumer)
                .arg("STREAMS")
                .arg(stream)
                .arg(">")
                .query_async::<_, Vec<Vec<(String, Vec<(String, HashMap<String, String>)>)>>>(&mut conn)
        ).await;
        
        match result {
            Ok(Ok(streams)) => {
                let mut messages = Vec::new();
                for stream_data in streams {
                    for (_, stream_messages) in stream_data {
                        for (id, fields) in stream_messages {
                            if let Some(data_json) = fields.get("data") {
                                if let Ok(message) = serde_json::from_str::<TestMessage>(data_json) {
                                    messages.push(message);
                                }
                            }
                        }
                    }
                }
                Ok(messages)
            },
            _ => Ok(vec![]),
        }
    }
}

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

#### Binary Performance Testing Setup
```javascript
// tests/performance/redis-streams-load-test.js
import { check } from 'k6';
import redis from 'k6/x/redis';

export let options = {
  stages: [
    { duration: '1m', target: 50 },   // Ramp up Redis connections
    { duration: '3m', target: 100 },  // Sustained load
    { duration: '1m', target: 0 },    // Ramp down
  ],
  thresholds: {
    redis_stream_publish_duration: ['p(95)<10'], // <10ms Redis Streams publish
    redis_stream_consume_duration: ['p(95)<10'], // <10ms Redis Streams consume
    stream_message_rate: ['rate>1000'],          // >1000 messages/sec
  },
};

const redisClient = new redis.Client('redis://localhost:6379');

export default function() {
  // Test Redis Streams performance
  const publishStart = Date.now();
  
  const messageData = {
    symbol: 'BTCUSD',
    price: 50000 + Math.random() * 1000,
    timestamp: Date.now(),
    source: 'load-test'
  };
  
  redisClient.xadd('market-data', '*', 'data', JSON.stringify(messageData));
  
  const publishDuration = Date.now() - publishStart;
  check(publishDuration, {
    'Redis publish under 10ms': (d) => d < 10,
  });
  
  // Test consumption
  const consumeStart = Date.now();
  const messages = redisClient.xread('STREAMS', 'market-data', '$');
  const consumeDuration = Date.now() - consumeStart;
  
  check(consumeDuration, {
    'Redis consume under 10ms': (d) => d < 10,
  });
}

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

## Binary-Specific Test Scripts

### Rust Binaries (`Makefile`)
```makefile
# Test all Rust binaries
test-rust:
	cargo test --workspace --all-features

# Test individual binaries
test-config-store:
	cargo test --bin config-store --features integration-tests

test-ruv-fann:
	cargo test --bin ruv-fann --features integration-tests

test-daa-coordinator:
	cargo test --bin daa-coordinator --features integration-tests

# Integration tests with Redis
test-integration-rust:
	docker-compose -f docker-compose.test.yml up -d redis-streams-test
	cargo test --features integration-tests -- --test-threads=1
	docker-compose -f docker-compose.test.yml down

# Performance tests
test-performance-rust:
	k6 run tests/performance/redis-streams-load-test.js
```

### Python Binary (`package.json` equivalent - `pyproject.toml`)
```toml
[tool.pytest.ini_options]
addopts = [
    "-v",
    "--cov=data_ingestion",
    "--cov-report=html",
    "--cov-report=term-missing",
    "--asyncio-mode=auto"
]
testpaths = ["tests/data-ingestion"]
markers = [
    "unit: Unit tests",
    "integration: Integration tests", 
    "redis: Redis integration tests",
    "slow: Slow running tests"
]

[build-system]
requires = ["setuptools", "wheel"]
build-backend = "setuptools.build_meta"
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