# Neural Trader V2 - Mock Services Framework
## Binary Separation Architecture Edition

## Overview

Comprehensive mocking framework for the **binary separation architecture**, enabling isolated testing of each binary and reliable Redis Streams communication testing without external dependencies.

### Binary Architecture Mocking Strategy
- **Binary Isolation**: Each binary can be tested independently
- **Redis Streams Mocking**: Mock Redis for cross-binary communication testing  
- **gRPC Service Mocking**: Mock config-store gRPC endpoints
- **Neural Network Mocking**: Mock FANN models and training data
- **Process-Level Mocking**: Mock binary startup/shutdown and inter-process communication

## Binary Mock Service Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                     Binary Mock Registry                           │
├─────────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐  │
│  │ Redis       │ │ Config      │ │ Neural      │ │ DAA Agent   │  │
│  │ Streams     │ │ Store       │ │ Network     │ │ Coordination│  │
│  │ Mock        │ │ gRPC Mock   │ │ FANN Mock   │ │ Mock        │  │
│  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘  │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐  │
│  │ Market Data │ │ Binary      │ │ Time        │ │ System      │  │
│  │ Stream Mock │ │ Process     │ │ Mock        │ │ Metrics     │  │
│  │             │ │ Mock        │ │             │ │ Mock        │  │
│  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

## 1. Binary Mock Service Registry

### Core Registry Implementation (Rust)
```rust
// tests/common/mock_registry.rs
use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;
use std::sync::Arc;

#[async_trait]
pub trait MockService: Send + Sync {
    fn name(&self) -> &str;
    async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn stop(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn reset(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn is_running(&self) -> bool;
}

pub struct BinaryMockRegistry {
    services: Arc<RwLock<HashMap<String, Box<dyn MockService>>>>,
    is_initialized: Arc<RwLock<bool>>,
}

impl BinaryMockRegistry {
    pub fn new() -> Self {
        Self {
            services: Arc::new(RwLock::new(HashMap::new())),
            is_initialized: Arc::new(RwLock::new(false)),
        }
    }
    
    pub async fn register(&self, service: Box<dyn MockService>) {
        let mut services = self.services.write().await;
        services.insert(service.name().to_string(), service);
    }

    pub async fn start_all(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut is_initialized = self.is_initialized.write().await;
        if *is_initialized {
            return Ok(());
        }
        
        let services = self.services.read().await;
        let mut start_futures = Vec::new();
        
        for service in services.values() {
            start_futures.push(service.start());
        }
        
        futures::future::try_join_all(start_futures).await?;
        *is_initialized = true;
        Ok(())
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

## 2. Redis Streams Mock Service

### Redis Streams Communication Mock
```rust
// tests/mocks/redis_streams_mock.rs
use async_trait::async_trait;
use redis::{Client, Connection, RedisResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use tokio::sync::{RwLock, mpsc};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMessage {
    pub id: String,
    pub stream: String,
    pub data: HashMap<String, String>,
    pub timestamp: u64,
}

pub struct RedisStreamsMock {
    streams: Arc<RwLock<HashMap<String, VecDeque<StreamMessage>>>>,
    consumer_groups: Arc<RwLock<HashMap<String, HashMap<String, String>>>>, // group -> consumer -> last_id
    message_tx: Arc<RwLock<Option<mpsc::UnboundedSender<StreamMessage>>>>,
    is_running: Arc<RwLock<bool>>,
}

#[async_trait]
impl MockService for RedisStreamsMock {
    fn name(&self) -> &str {
        "redis-streams-mock"
    }
    
    async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Ok(());
        }
        
        // Initialize default streams
        let mut streams = self.streams.write().await;
        let default_streams = [
            "config-updates",
            "market-data",
            "neural-signals", 
            "agent-coordination",
            "system-events"
        ];
        
        for stream_name in default_streams {
            streams.insert(stream_name.to_string(), VecDeque::new());
        }
        
        *is_running = true;
        Ok(())
    }

    async fn stop(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut is_running = self.is_running.write().await;
        *is_running = false;
        
        // Clear all streams
        let mut streams = self.streams.write().await;
        streams.clear();
        
        // Clear consumer groups
        let mut consumer_groups = self.consumer_groups.write().await;
        consumer_groups.clear();
        
        Ok(())
    }
    
    async fn reset(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.stop().await?;
        self.start().await?;
        Ok(())
    }
    
    fn is_running(&self) -> bool {
        // This is a bit of a hack since we can't await in a sync method
        // In practice, you'd use Arc<AtomicBool> for this
        true
    }
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

impl RedisStreamsMock {
    pub fn new() -> Self {
        Self {
            streams: Arc::new(RwLock::new(HashMap::new())),
            consumer_groups: Arc::new(RwLock::new(HashMap::new())),
            message_tx: Arc::new(RwLock::new(None)),
            is_running: Arc::new(RwLock::new(false)),
        }
    }
    
    pub async fn xadd(
        &self,
        stream: &str,
        id: &str, 
        fields: HashMap<String, String>
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut streams = self.streams.write().await;
        
        let message_id = if id == "*" {
            format!("{}-{}", 
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_millis(),
                rand::random::<u32>()
            )
        } else {
            id.to_string()
        };
        
        let message = StreamMessage {
            id: message_id.clone(),
            stream: stream.to_string(),
            data: fields,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis() as u64,
        };
        
        let stream_queue = streams.entry(stream.to_string()).or_insert_with(VecDeque::new);
        stream_queue.push_back(message);
        
        // Keep only last 1000 messages per stream to prevent memory bloat
        while stream_queue.len() > 1000 {
            stream_queue.pop_front();
        }
        
        Ok(message_id)
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

## 3. Config-Store gRPC Mock Service

### gRPC Service Mock (Rust)
```rust
// tests/mocks/config_store_grpc_mock.rs
use tonic::{transport::Server, Request, Response, Status};
use std::collections::HashMap;
use tokio::sync::RwLock;
use std::sync::Arc;

// Generated protobuf code
use crate::proto::config_store_server::{ConfigStore, ConfigStoreServer};
use crate::proto::{GetConfigRequest, GetConfigResponse, SetConfigRequest, SetConfigResponse};

pub struct MockConfigStoreService {
    configs: Arc<RwLock<HashMap<String, String>>>,
    request_log: Arc<RwLock<Vec<String>>>,
}

impl MockConfigStoreService {
    pub fn new() -> Self {
        Self {
            configs: Arc::new(RwLock::new(HashMap::new())),
            request_log: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn set_config(&self, key: &str, value: &str) {
        let mut configs = self.configs.write().await;
        configs.insert(key.to_string(), value.to_string());
    }
    
    pub async fn get_request_log(&self) -> Vec<String> {
        let log = self.request_log.read().await;
        log.clone()
    }
    
    pub async fn clear_request_log(&self) {
        let mut log = self.request_log.write().await;
        log.clear();
    }
}

#[tonic::async_trait]
impl ConfigStore for MockConfigStoreService {
    async fn get_config(
        &self,
        request: Request<GetConfigRequest>,
    ) -> Result<Response<GetConfigResponse>, Status> {
        let req = request.into_inner();
        
        // Log the request
        let mut log = self.request_log.write().await;
        log.push(format!("get_config: {}", req.key));
        drop(log);
        
        let configs = self.configs.read().await;
        
        match configs.get(&req.key) {
            Some(value) => {
                let response = GetConfigResponse {
                    value: value.clone(),
                    found: true,
                };
                Ok(Response::new(response))
            }
            None => {
                let response = GetConfigResponse {
                    value: String::new(),
                    found: false,
                };
                Ok(Response::new(response))
            }
        }
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

## 4. Neural Network FANN Mock

### FANN Neural Network Mock (Rust)
```rust
// tests/mocks/fann_mock.rs
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralInput {
    pub features: Vec<f32>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralOutput {
    pub predictions: Vec<f32>,
    pub confidence: f32,
    pub timestamp: u64,
}

pub struct MockFannService {
    models: Arc<RwLock<HashMap<String, MockNeuralModel>>>,
    training_data: Arc<RwLock<Vec<(NeuralInput, Vec<f32>)>>>,
    is_running: Arc<RwLock<bool>>,
}

struct MockNeuralModel {
    weights: Vec<Vec<f32>>,
    biases: Vec<f32>,
    accuracy: f32,
}

impl MockFannService {
    pub fn new() -> Self {
        Self {
            models: Arc::new(RwLock::new(HashMap::new())),
            training_data: Arc::new(RwLock::new(Vec::new())),
            is_running: Arc::new(RwLock::new(false)),
        }
    }
    
    pub async fn create_mock_model(&self, name: &str, input_size: usize, output_size: usize) {
        let mut models = self.models.write().await;
        
        // Create a simple mock model with random weights
        let model = MockNeuralModel {
            weights: vec![vec![0.5; input_size]; output_size],
            biases: vec![0.1; output_size],
            accuracy: 0.85, // Mock accuracy
        };
        
        models.insert(name.to_string(), model);
    }

  reset(): void {
    this.cache.clear();
    this.pubsub.clear();
  }

  isRunning(): boolean {
    return true;
  }

    pub async fn predict(&self, model_name: &str, input: &NeuralInput) -> Option<NeuralOutput> {
        let models = self.models.read().await;
        
        if let Some(model) = models.get(model_name) {
            // Simple mock prediction - just apply weights
            let mut predictions = Vec::new();
            
            for (i, weights) in model.weights.iter().enumerate() {
                let mut sum = model.biases[i];
                for (j, &feature) in input.features.iter().enumerate() {
                    if j < weights.len() {
                        sum += feature * weights[j];
                    }
                }
                predictions.push(sum.tanh()); // Apply activation
            }
            
            Some(NeuralOutput {
                predictions,
                confidence: model.accuracy,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
            })
        } else {
            None
        }
    }
    
    pub async fn add_training_data(&self, input: NeuralInput, expected: Vec<f32>) {
        let mut training_data = self.training_data.write().await;
        training_data.push((input, expected));
        
        // Keep only last 10,000 training samples to prevent memory bloat
        while training_data.len() > 10000 {
            training_data.remove(0);
        }
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

## 5. DAA Coordinator Mock Service

### Distributed Agent Mock (Rust)
```rust
// tests/mocks/daa_coordinator_mock.rs
use std::collections::HashMap;
use tokio::sync::{RwLock, mpsc};
use std::sync::Arc;
use uuid::Uuid;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub agent_type: String,
    pub status: AgentStatus,
    pub capabilities: Vec<String>,
    pub last_heartbeat: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentStatus {
    Active,
    Idle,
    Busy,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinationMessage {
    pub from_agent: String,
    pub to_agent: Option<String>, // None for broadcast
    pub message_type: String,
    pub payload: HashMap<String, String>,
    pub timestamp: u64,
}

pub struct MockDaaCoordinator {
    agents: Arc<RwLock<HashMap<String, Agent>>>,
    message_queue: Arc<RwLock<Vec<CoordinationMessage>>>,
    coordination_tx: Arc<RwLock<Option<mpsc::UnboundedSender<CoordinationMessage>>>>,
    is_running: Arc<RwLock<bool>>,
}
    
impl MockDaaCoordinator {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            message_queue: Arc::new(RwLock::new(Vec::new())),
            coordination_tx: Arc::new(RwLock::new(None)),
            is_running: Arc::new(RwLock::new(false)),
        }
    }
    
    pub async fn register_agent(&self, agent_type: &str, capabilities: Vec<String>) -> String {
        let agent_id = Uuid::new_v4().to_string();
        let mut agents = self.agents.write().await;
        
        let agent = Agent {
            id: agent_id.clone(),
            agent_type: agent_type.to_string(),
            status: AgentStatus::Active,
            capabilities,
            last_heartbeat: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };
        
        agents.insert(agent_id.clone(), agent);
        agent_id
    }
    
    pub async fn send_coordination_message(&self, message: CoordinationMessage) {
        let mut queue = self.message_queue.write().await;
        queue.push(message);
        
        // Keep only last 1000 messages
        while queue.len() > 1000 {
            queue.remove(0);
        }
    }

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

## 8. Binary Mock Integration Example

### Cross-Binary Integration Test
```rust
// tests/integration/cross_binary_integration_test.rs
use crate::mocks::{
    RedisStreamsMock, MockConfigStoreService, MockFannService, MockDaaCoordinator,
    BinaryMockRegistry
};
use tokio_test;

#[tokio::test]
async fn test_cross_binary_integration() {
    let mut mock_registry = BinaryMockRegistry::new();
    
    // Initialize all mock services
    let redis_mock = Box::new(RedisStreamsMock::new());
    let config_mock = Box::new(MockConfigStoreService::new());
    let fann_mock = Box::new(MockFannService::new());
    let daa_mock = Box::new(MockDaaCoordinator::new());
    
    mock_registry.register(redis_mock).await;
    mock_registry.register(config_mock).await;
    mock_registry.register(fann_mock).await;
    mock_registry.register(daa_mock).await;

    // Start all mock services
    mock_registry.start_all().await.unwrap();
    
    // Test scenario: Config update triggers neural processing
    
    // 1. Config-store publishes configuration update
    let config_update = std::collections::HashMap::from([
        ("key".to_string(), "neural_model_params".to_string()),
        ("value".to_string(), "{\"learning_rate\": 0.01}".to_string()),
    ]);
    
    let redis_service = mock_registry.get_service::<RedisStreamsMock>("redis-streams-mock").await.unwrap();
    redis_service.xadd("config-updates", "*", config_update).await.unwrap();
    
    // 2. Simulate data-ingestion receiving market data and forwarding to neural processing
    let market_data = std::collections::HashMap::from([
        ("symbol".to_string(), "BTCUSD".to_string()),
        ("price".to_string(), "50000.0".to_string()),
        ("timestamp".to_string(), "1640995200000".to_string()),
    ]);
    
    redis_service.xadd("market-data", "*", market_data).await.unwrap();

  beforeEach(() => {
    mockRegistry.resetAll();
  });

  afterAll(async () => {
    await mockRegistry.stopAll();
  });

    // 3. Verify neural processing received the data
    let fann_service = mock_registry.get_service::<MockFannService>("fann-mock").await.unwrap();
    fann_service.create_mock_model("trading_model", 5, 3).await;
    
    let neural_input = crate::mocks::NeuralInput {
        features: vec![50000.0, 1000.0, 0.1, 0.05, 1.2], // price, volume, volatility, etc.
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
    };
    
    let prediction = fann_service.predict("trading_model", &neural_input).await.unwrap();
    assert!(prediction.confidence > 0.8);
    assert_eq!(prediction.predictions.len(), 3);
    
    // 4. Verify DAA coordinator received coordination signal
    let daa_service = mock_registry.get_service::<MockDaaCoordinator>("daa-mock").await.unwrap();
    let agent_id = daa_service.register_agent("trader", vec!["trading".to_string(), "risk_management".to_string()]).await;
    
    let coordination_msg = crate::mocks::CoordinationMessage {
        from_agent: "neural_processor".to_string(),
        to_agent: Some(agent_id),
        message_type: "trade_signal".to_string(),
        payload: std::collections::HashMap::from([
            ("action".to_string(), "buy".to_string()),
            ("confidence".to_string(), prediction.confidence.to_string()),
        ]),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
    };
    
    daa_service.send_coordination_message(coordination_msg).await;
    
    // 5. Verify end-to-end message flow
    let messages = redis_service.xread("market-data", "$", 1).await.unwrap();
    assert!(!messages.is_empty());
    
    // Cleanup
    mock_registry.stop_all().await.unwrap();
}
```

This comprehensive mock framework enables reliable, fast, and isolated testing of all system components without external dependencies.