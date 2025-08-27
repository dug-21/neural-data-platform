# EventBus Proto Contract Enforcement with Data-Staging - SPARC Planning Artifacts

## Overview
This directory contains comprehensive SPARC planning artifacts for enforcing Protocol Buffer contracts through a new Data-Staging service that bridges raw data to the proto-only EventBus. These documents establish a clear architectural separation: raw data in Redis, structured proto in EventBus.

## Problem Statement
The current architecture has multiple issues:
- Data-Ingestion publishes raw JSON to Redis (this is correct - it's raw data)
- EventBus would need to handle both raw and structured data (violates single responsibility)
- No clear quality gate between raw external data and internal structured messaging
- Proto contracts exist but aren't enforced at the right boundary

## Solution: Data-Staging Service + Proto-Only EventBus
The architecture now clearly separates concerns:
1. **Data-Ingestion** → Redis (Raw JSON) - unchanged
2. **Data-Staging** (NEW) → Validates, transforms, and enriches raw data to proto
3. **EventBus** → Proto-only message bus for structured data
4. **Consumers** → Receive only validated, structured proto messages

### 📋 Core Planning Documents

#### 1. [SPECIFICATION](./1_SPECIFICATION.md)
- Strict proto-only requirements
- Zero tolerance for non-proto messages
- 100% proto compliance success criteria
- Mandatory contract enforcement

#### 2. [PSEUDOCODE](./2_PSEUDOCODE.md)
- Proto validation algorithms that REJECT non-proto data
- Fail-fast error handling with no fallbacks
- Direct protobuf processing (no Vec<u8> transformation)
- Strict schema compliance enforcement
- Immediate rejection of malformed messages

#### 3. [ARCHITECTURE](./3_ARCHITECTURE.md)
- Single proto-only path (no dual support)
- Contract Guard as first line of defense
- Strict validation at every layer
- No compatibility layers or fallbacks
- Proto contracts as mandatory foundation
- Simplified architecture without conversion overhead

### 🔧 Implementation Planning

#### [BUILD_CONFIGURATION](./implementation/BUILD_CONFIGURATION.md)
- Mandatory proto compilation (NOT optional)
- Build FAILS if proto compilation fails
- No feature flags - proto is always required
- Proto validation at compile time

#### [EVENT_TYPE_DESIGN](./implementation/EVENT_TYPE_DESIGN.md)
- Event type directly wraps EventEnvelope
- NO Vec<u8> support or conversion
- NO backward compatibility
- Strict type safety with zero escape hatches

#### [GRPC_SERVICES](./implementation/GRPC_SERVICES.md)
- Services ONLY accept valid protobuf
- Immediate rejection of non-proto data
- No fallback handlers or compatibility modes
- Contract violations as fatal errors

### 🧪 Testing & Quality

#### [TESTING_STRATEGY](./tests/TESTING_STRATEGY.md)
- Tests that verify Vec<u8> messages are REJECTED
- Strict schema validation testing
- Contract enforcement verification
- Negative tests for non-proto data
- Proto-only performance benchmarks

### 📚 Documentation

#### [MIGRATION_GUIDE](./docs/MIGRATION_GUIDE.md)
- Simple directive: Convert to proto or it won't work
- No phased migration or dual support
- Complete service rewrites required
- No backward compatibility
- Proto compliance is mandatory

### 📅 [INTEGRATION_TIMELINE](./INTEGRATION_TIMELINE.md)
- **5-6 week implementation** (includes Data-Staging development)
- **227 developer hours** (includes 80 hours for Data-Staging)
- Clean separation between raw data and structured messaging
- Data-Staging as quality gate and transformation layer
- Proto-only EventBus with strict enforcement

## Key Technical Decisions

### Proto Contracts Are Mandatory
**ALL messages MUST be protobuf** - no exceptions, no fallbacks, no legacy support.

### Proto Files to Enforce
**Core Protos** (`/proto/`):
- `common.proto` - Shared types
- `market_data.proto` - Market data events
- `trading.proto` - Trading operations
- `features.proto` - ML feature extraction
- `models.proto` - ML model definitions
- `config_store.proto` - Configuration management

**EventBus Schemas** (`/schemas/`):
- `ingestion-eventbus.proto` - Data ingestion interface
- `eventbus-mlops.proto` - ML-Ops interface
- `mlops-execution.proto` - Execution interface
- `execution-action.proto` - Action layer interface

### Performance Improvements (Proto-Only)
- **Throughput**: 100K-500K messages/second
- **Latency**: <50ms p99
- **CPU**: 20-40% reduction vs dual support
- **Memory**: 30-50% reduction without conversion overhead

### Migration Strategy
**IMMEDIATE COMPLIANCE** - No gradual migration:
1. Update all services to proto
2. Deploy proto-only EventBus
3. System rejects any non-proto messages

## Implementation Phases

### Phase 1: Data-Staging Foundation (Week 1)
- Create Data-Staging service
- Implement Redis consumer for raw JSON
- Build JSON to proto transformation

### Phase 2: Data-Staging Integration (Week 2)
- Connect Data-Staging to EventBus
- Implement quality scoring
- Add DLQ for invalid data

### Phase 3: EventBus Proto-Only (Week 3)
- Update EventBus to reject all non-proto
- Remove any JSON compatibility code
- Enforce strict proto validation

### Phase 4: Consumer Migration (Week 4)
- Update all consumers to proto
- Remove JSON parsing from ML-Ops
- Update Execution for proto messages

### Phase 5: Production Hardening (Week 5)
- End-to-end testing
- Performance optimization
- Monitoring and alerting

### Phase 6: Deployment (Week 6)
- Deploy Data-Staging service
- Monitor quality metrics
- Complete migration

## Success Metrics
- ✅ **100% proto compliance** - ZERO non-proto messages allowed
- ✅ **All Vec<u8> attempts rejected** with clear errors
- ✅ **Contract violations result in system failure** (fail-fast)
- ✅ **No backward compatibility** - proto is mandatory
- ✅ **Performance improvement** from eliminating conversion overhead
- ✅ **Type safety guaranteed** at compile time

## Next Steps
1. Review and approve planning artifacts
2. Allocate development resources
3. Begin Phase 1 implementation
4. Set up CI/CD pipeline for proto compilation


## References
- [Phase 4 EventBus Specification](../README.md)
- [MVP Architecture](../../mvp/architecture/)
- [Proto Files](/workspaces/neural-trader/proto/)
- [Schema Definitions](/workspaces/neural-trader/schemas/)

---

*Generated by Neural-Trader SPARC Planning Process*
*Last Updated: 2025-01-26*