# EventBus Proto Integration Migration Guide

## Overview

This guide provides instructions for migrating EventBus consumers from JSON-based messaging to Protocol Buffers (protobuf) integration. **This is a proto-only implementation with no backward compatibility.**

**IMPORTANT**: After migration, all messages must use Protocol Buffers. JSON messages will be rejected.

## Table of Contents

1. [Migration Strategy](#migration-strategy)
2. [Step-by-Step Migration](#step-by-step-migration)
3. [Code Examples](#code-examples)
4. [Common Pitfalls](#common-pitfalls)
5. [Performance Tuning](#performance-tuning)
6. [Debugging Proto Issues](#debugging-proto-issues)

## Migration Strategy

### Proto-Only Approach

**This is a complete migration to Protocol Buffers with no backward compatibility:**

1. Convert all event definitions to proto format
2. Update all publishers to emit proto messages
3. Update all consumers to handle proto messages
4. Deploy the updated system

**No dual-format period. No JSON fallback. Use proto or your messages will be rejected.**

## Migration Steps (UPDATED)

### Phase 1: Deploy Data-Staging Service
1. **Deploy Data-Staging** alongside existing infrastructure
2. **Configure Redis subscription** to raw data channels
3. **Verify proto conversion** works correctly
4. **Monitor quality metrics** 

### Phase 2: Update EventBus Consumers
1. **All consumers MUST** expect proto-only messages
2. **Remove any JSON parsing** code from consumers
3. **Update to use generated proto classes**

### Phase 3: Cut Over
1. **Data-Staging goes live** consuming from Redis
2. **EventBus accepts ONLY** proto from Data-Staging
3. **Stop any direct Redis→EventBus paths**
4. **Monitor for rejected messages**

## Architecture After Migration
```
Data-Ingestion (unchanged) → Redis (raw JSON)
                                ↓
                        Data-Staging (NEW)
                                ↓ (proto only)
                            EventBus
                                ↓
                        All Consumers (proto)
```

## Key Points
- Data-Ingestion remains unchanged (still publishes JSON)
- Data-Staging is the ONLY path to EventBus
- EventBus is proto-only, no exceptions
- Clear separation: raw data (Redis) vs structured (EventBus)

## Step-by-Step Migration

### Prerequisites

1. Install proto dependencies:
```bash
npm install protobufjs @types/protobufjs
npm install --save-dev @protobuf-ts/plugin
```

2. Generate proto files:
```bash
# Generate TypeScript definitions
npx protoc --ts_out=src/generated --proto_path=proto proto/**/*.proto
```

3. Update build configuration to include proto generation.

### Step 1: Update Event Definitions

**Before (JSON)**:
```typescript
// events/market-data.ts
export interface MarketDataEvent {
  symbol: string;
  price: number;
  timestamp: number;
  volume: number;
}
```

**After (Proto Only)**:
```typescript
// events/market-data.ts
import { MarketDataEvent as ProtoMarketDataEvent } from '../generated/market_data_pb';

export class MarketDataEvent {
  constructor(
    public symbol: string,
    public price: number,
    public timestamp: number,
    public volume: number
  ) {}

  toProto(): ProtoMarketDataEvent {
    const proto = new ProtoMarketDataEvent();
    proto.setSymbol(this.symbol);
    proto.setPrice(this.price);
    proto.setTimestamp(this.timestamp);
    proto.setVolume(this.volume);
    return proto;
  }

  static fromProto(proto: ProtoMarketDataEvent): MarketDataEvent {
    return new MarketDataEvent(
      proto.getSymbol(),
      proto.getPrice(),
      proto.getTimestamp(),
      proto.getVolume()
    );
  }
}
```

### Step 2: Update EventBus Publisher

**Before**:
```typescript
// publishers/market-data-publisher.ts
export class MarketDataPublisher {
  async publish(event: MarketDataEvent): Promise<void> {
    await this.eventBus.emit('market.data.updated', event);
  }
}
```

**After (Proto Only)**:
```typescript
// publishers/market-data-publisher.ts
export class MarketDataPublisher {
  constructor(private eventBus: EventBus) {}

  async publish(event: MarketDataEvent): Promise<void> {
    // Proto format only
    const protoData = event.toProto().serializeBinary();
    await this.eventBus.emit('market.data.updated', {
      format: 'proto',
      data: protoData,
      schema: 'MarketDataEvent',
      version: '1.0.0'
    });
  }
}
```

### Step 3: Update EventBus Consumer

**Before**:
```typescript
// consumers/market-data-consumer.ts
export class MarketDataConsumer {
  constructor(private eventBus: EventBus) {
    this.eventBus.on('market.data.updated', this.handleMarketData.bind(this));
  }

  private async handleMarketData(event: MarketDataEvent): Promise<void> {
    console.log(`Received market data for ${event.symbol}: $${event.price}`);
    // Process event...
  }
}
```

**After (Proto Only)**:
```typescript
// consumers/market-data-consumer.ts
import { ProtoEventBus } from '../infrastructure/proto-event-bus';
import { MarketDataEvent as ProtoMarketDataEvent } from '../generated/market_data_pb';

export class MarketDataConsumer {
  constructor(private protoEventBus: ProtoEventBus) {
    this.setupEventHandlers();
  }

  private setupEventHandlers(): void {
    // Proto handler only
    this.protoEventBus.subscribe('market.data.updated', this.handleProtoMarketData.bind(this));
  }

  private async handleProtoMarketData(eventWrapper: ProtoEventWrapper): Promise<void> {
    try {
      const protoEvent = ProtoMarketDataEvent.deserializeBinary(eventWrapper.data);
      const event = MarketDataEvent.fromProto(protoEvent);
      await this.processMarketData(event);
    } catch (error) {
      console.error('Failed to deserialize proto event:', error);
      // No fallback - proto is required
      throw error;
    }
  }

  private async processMarketData(event: MarketDataEvent): Promise<void> {
    console.log(`Received market data for ${event.symbol}: $${event.price}`);
    // Process event...
  }
}
```

### Step 4: Update Service Registration

**Before**:
```typescript
// services/service-container.ts
container.register('MarketDataConsumer', MarketDataConsumer);
```

**After (Proto Only)**:
```typescript
// services/service-container.ts
container.register('MarketDataConsumer', MarketDataConsumer, [
  'ProtoEventBus'
]);
```

## Configuration

No feature flags needed. Proto is the only supported format.

```typescript
// config/proto-config.ts
export interface ProtoConfig {
  validation: boolean;
  metrics: boolean;
  compression: boolean;
}

export const protoConfig: ProtoConfig = {
  validation: process.env.ENABLE_PROTO_VALIDATION !== 'false',
  metrics: process.env.ENABLE_PROTO_METRICS !== 'false',
  compression: process.env.ENABLE_PROTO_COMPRESSION === 'true'
};
```

## Common Pitfalls

### 1. Schema Evolution Issues

**Problem**: Adding new fields breaks old consumers

**Solution**: Always use optional fields in proto definitions
```protobuf
message MarketDataEvent {
  string symbol = 1;
  double price = 2;
  int64 timestamp = 3;
  double volume = 4;
  // New fields should be optional
  optional double bid_price = 5;
  optional double ask_price = 6;
}
```

### 2. Serialization Errors

**Problem**: Proto serialization fails

**Solution**: Add validation and proper error handling
```typescript
private serializeEvent(event: MarketDataEvent): Uint8Array {
  try {
    const proto = event.toProto();
    
    // Validate required fields
    if (!proto.getSymbol() || !proto.getPrice()) {
      throw new Error('Required fields missing');
    }
    
    return proto.serializeBinary();
  } catch (error) {
    console.error('Proto serialization failed:', error);
    // No fallback - proto is required
    throw new SerializationError('Failed to serialize event', error);
  }
}
```

### 3. Performance Degradation

**Problem**: Proto deserialization is slower than expected

**Solution**: Implement object pooling and caching
```typescript
class ProtoObjectPool {
  private pool: Map<string, any[]> = new Map();

  borrow<T>(type: string, creator: () => T): T {
    const objects = this.pool.get(type) || [];
    return objects.pop() || creator();
  }

  return<T>(type: string, object: T): void {
    const objects = this.pool.get(type) || [];
    objects.push(object);
    this.pool.set(type, objects);
  }
}
```

### 4. Memory Leaks

**Problem**: Proto objects not properly released

**Solution**: Implement proper cleanup
```typescript
class ProtoEventHandler {
  private cleanup = new Set<() => void>();

  async handleEvent(protoData: Uint8Array): Promise<void> {
    const event = ProtoMarketDataEvent.deserializeBinary(protoData);
    
    // Register cleanup
    this.cleanup.add(() => {
      // Clean up proto object if needed
    });

    try {
      await this.processEvent(event);
    } finally {
      // Execute cleanup
      this.cleanup.forEach(fn => fn());
      this.cleanup.clear();
    }
  }
}
```

## Performance Tuning

### 1. Batch Processing

```typescript
class BatchProtoProcessor {
  private batchSize = 100;
  private batch: Uint8Array[] = [];

  async addToBatch(protoData: Uint8Array): Promise<void> {
    this.batch.push(protoData);
    
    if (this.batch.length >= this.batchSize) {
      await this.processBatch();
    }
  }

  private async processBatch(): Promise<void> {
    const events = this.batch.map(data => 
      ProtoMarketDataEvent.deserializeBinary(data)
    );
    
    await Promise.all(events.map(event => this.processEvent(event)));
    this.batch = [];
  }
}
```

### 2. Compression

```typescript
import * as zlib from 'zlib';

class CompressedProtoPublisher {
  async publish(event: MarketDataEvent): Promise<void> {
    const protoData = event.toProto().serializeBinary();
    const compressed = zlib.gzipSync(protoData);
    
    await this.eventBus.emit('market.data.updated', {
      format: 'proto',
      data: compressed,
      compressed: true,
      schema: 'MarketDataEvent'
    });
  }
}
```

### 3. Connection Pooling

```typescript
class ProtoEventBusPool {
  private connections: ProtoEventBus[] = [];
  private currentIndex = 0;

  constructor(poolSize: number = 5) {
    for (let i = 0; i < poolSize; i++) {
      this.connections.push(new ProtoEventBus());
    }
  }

  getConnection(): ProtoEventBus {
    const connection = this.connections[this.currentIndex];
    this.currentIndex = (this.currentIndex + 1) % this.connections.length;
    return connection;
  }
}
```

## Debugging Proto Issues

### 1. Enable Debug Logging

```typescript
// config/logging.ts
export const protoLoggerConfig = {
  level: process.env.PROTO_LOG_LEVEL || 'info',
  enableSerialization: process.env.ENABLE_PROTO_SERIALIZATION_LOG === 'true',
  enableDeserialization: process.env.ENABLE_PROTO_DESERIALIZATION_LOG === 'true'
};
```

### 2. Proto Event Inspector

```typescript
class ProtoEventInspector {
  inspect(protoData: Uint8Array, schema: string): void {
    console.log(`=== Proto Event Inspection ===`);
    console.log(`Schema: ${schema}`);
    console.log(`Data Size: ${protoData.length} bytes`);
    
    try {
      const event = this.deserializeBySchema(protoData, schema);
      console.log(`Deserialized Event:`, JSON.stringify(event, null, 2));
    } catch (error) {
      console.error(`Deserialization Error:`, error);
      console.log(`Raw Data:`, Array.from(protoData).map(b => b.toString(16)).join(' '));
    }
  }

  private deserializeBySchema(data: Uint8Array, schema: string): any {
    switch (schema) {
      case 'MarketDataEvent':
        return ProtoMarketDataEvent.deserializeBinary(data);
      // Add other schemas...
      default:
        throw new Error(`Unknown schema: ${schema}`);
    }
  }
}
```

### 3. Health Check Endpoints

```typescript
// health/proto-health.ts
export class ProtoHealthChecker {
  async checkHealth(): Promise<HealthStatus> {
    const status: HealthStatus = {
      proto_eventbus: 'healthy',
      serialization: 'healthy',
      deserialization: 'healthy',
      performance: 'healthy'
    };

    try {
      // Test serialization
      const testEvent = new MarketDataEvent('TEST', 100, Date.now(), 1000);
      const protoData = testEvent.toProto().serializeBinary();
      
      // Test deserialization
      const deserializedEvent = ProtoMarketDataEvent.deserializeBinary(protoData);
      
      // Performance test
      const startTime = performance.now();
      for (let i = 0; i < 1000; i++) {
        testEvent.toProto().serializeBinary();
      }
      const duration = performance.now() - startTime;
      
      if (duration > 100) { // 100ms threshold
        status.performance = 'warning';
      }
      
    } catch (error) {
      status.proto_eventbus = 'unhealthy';
      status.serialization = 'unhealthy';
    }

    return status;
  }
}
```

## Rollback Procedures

### 1. Emergency Rollback

```bash
#!/bin/bash
# scripts/emergency-rollback.sh

echo "Initiating emergency rollback..."

# Disable proto feature flags
export ENABLE_PROTO_EVENTBUS=false
export ENABLE_PROTO_FALLBACK=false

# Restart services
docker-compose restart neural-trader-api
docker-compose restart neural-trader-consumer

echo "Rollback completed. System running on JSON EventBus."
```

### 2. Gradual Rollback

```typescript
// services/rollback-manager.ts
export class RollbackManager {
  async initiateGradualRollback(): Promise<void> {
    console.log('Starting gradual rollback...');
    
    // Phase 1: Reduce proto traffic to 50%
    await this.updateFeatureFlag('proto_rollout_percentage', 50);
    await this.waitAndMonitor(300000); // 5 minutes
    
    // Phase 2: Reduce to 10%
    await this.updateFeatureFlag('proto_rollout_percentage', 10);
    await this.waitAndMonitor(300000);
    
    // Phase 3: Disable completely
    await this.updateFeatureFlag('proto_eventbus', false);
    
    console.log('Gradual rollback completed.');
  }

  private async updateFeatureFlag(flag: string, value: any): Promise<void> {
    // Update feature flag in configuration service
    await this.configService.updateFlag(flag, value);
    
    // Notify all services
    await this.eventBus.emit('config.updated', { flag, value });
  }

  private async waitAndMonitor(duration: number): Promise<void> {
    return new Promise(resolve => {
      setTimeout(async () => {
        const health = await this.healthChecker.checkHealth();
        if (health.proto_eventbus === 'unhealthy') {
          throw new Error('System unhealthy during rollback');
        }
        resolve();
      }, duration);
    });
  }
}
```

### 3. Proto Validation

```typescript
class ProtoValidator {
  async validateProtoEvents(): Promise<boolean> {
    console.log('Validating proto events...');
    
    const testEvents = [
      new MarketDataEvent('AAPL', 150.25, Date.now(), 1000),
      new MarketDataEvent('GOOGL', 2800.50, Date.now(), 500),
    ];

    for (const event of testEvents) {
      try {
        // Test serialization
        const protoData = event.toProto().serializeBinary();
        
        // Test deserialization
        const deserializedProto = ProtoMarketDataEvent.deserializeBinary(protoData);
        const deserializedEvent = MarketDataEvent.fromProto(deserializedProto);
        
        // Validate data integrity
        if (!this.validateEventData(event, deserializedEvent)) {
          console.error('Proto validation failed for event:', event);
          return false;
        }
      } catch (error) {
        console.error('Proto processing failed:', error);
        return false;
      }
    }

    console.log('Proto validation passed.');
    return true;
  }

  private validateEventData(original: MarketDataEvent, deserialized: MarketDataEvent): boolean {
    return original.symbol === deserialized.symbol &&
           original.price === deserialized.price &&
           original.timestamp === deserialized.timestamp &&
           original.volume === deserialized.volume;
  }
}
```


## Migration Checklist

### Pre-Migration
- [ ] Review proto schema definitions
- [ ] Install proto dependencies
- [ ] Test in development environment
- [ ] Plan deployment strategy
- [ ] Update monitoring for proto-only

### Migration
- [ ] Convert all event definitions to proto
- [ ] Update all publishers to emit proto messages
- [ ] Update all consumers to handle proto messages
- [ ] Remove all JSON handling code
- [ ] Deploy proto-only system

### Post-Migration
- [ ] Validate all events are proto format
- [ ] Monitor performance metrics
- [ ] Update documentation
- [ ] Update monitoring dashboards
- [ ] Train team on proto debugging

## Support and Resources

- **Documentation**: `/docs/proto/`
- **Examples**: `/examples/proto-migration/`
- **Testing**: `/tests/integration/proto/`
- **Monitoring**: Grafana dashboard "Proto EventBus"
- **Alerts**: Slack channel #proto-migration
- **On-call**: EventBus team rotation

## Conclusion

This migration guide provides a direct approach to adopting proto-only integration in the EventBus system. **There is no backward compatibility - all consumers must be updated to use Protocol Buffers before deployment.**

Key points:
- **No JSON support** - proto format is required
- **No fallback mechanisms** - messages must be valid proto
- **Complete migration** - update all publishers and consumers
- **Single deployment** - no gradual rollout needed

For questions or issues during migration, please contact the EventBus team or create an issue in the project repository.