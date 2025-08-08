# Phase 2 Data-Ingestion Python Success Criteria

## Overview

This document defines the measurable success criteria for the Python data-ingestion service migration from unified Redis channel publishing to per-symbol channel publishing.

## Performance Metrics

### 1. Publishing Performance

#### 1.1 Latency Requirements
```python
# Target latency measurements (95th percentile)
PERFORMANCE_TARGETS = {
    "redis_publish_latency_ms": {
        "target": 2.0,           # < 2ms per publish
        "acceptable": 5.0,       # < 5ms acceptable
        "critical": 10.0         # > 10ms critical issue
    },
    "end_to_end_processing_ms": {
        "target": 50.0,          # < 50ms market data to Redis
        "acceptable": 100.0,     # < 100ms acceptable  
        "critical": 250.0        # > 250ms critical issue
    },
    "channel_routing_overhead_ms": {
        "target": 0.1,           # < 0.1ms channel resolution
        "acceptable": 0.5,       # < 0.5ms acceptable
        "critical": 2.0          # > 2ms critical issue
    }
}
```

#### 1.2 Throughput Requirements
```python
THROUGHPUT_TARGETS = {
    "messages_per_second": {
        "single_symbol": 10000,   # 10k msg/sec per symbol
        "multi_symbol_100": 50000, # 50k msg/sec across 100 symbols
        "multi_symbol_1000": 100000 # 100k msg/sec across 1000 symbols
    },
    "concurrent_symbols": {
        "target": 1000,           # Support 1000 concurrent symbols
        "acceptable": 500,        # 500 symbols acceptable
        "minimum": 100            # 100 symbols minimum
    }
}
```

### 2. Resource Utilization

#### 2.1 Memory Usage
```python
MEMORY_TARGETS = {
    "baseline_memory_mb": {
        "current": "TBD",         # Baseline measurement needed
        "increase_limit": 50,     # < 50MB increase allowed
        "critical_limit": 100     # > 100MB increase critical
    },
    "per_symbol_overhead_kb": {
        "target": 10,             # < 10KB per active symbol
        "acceptable": 50,         # < 50KB acceptable
        "critical": 200           # > 200KB critical
    },
    "connection_pool_efficiency": {
        "reuse_rate": 0.95,       # > 95% connection reuse
        "max_connections": 50,    # Stay within current pool size
        "idle_timeout": 300       # 5 minute idle timeout
    }
}
```

#### 2.2 CPU Usage
```python
CPU_TARGETS = {
    "processing_overhead": {
        "target": 5,              # < 5% CPU increase
        "acceptable": 10,         # < 10% acceptable
        "critical": 20            # > 20% critical
    },
    "async_efficiency": {
        "context_switches": "minimal",  # No excessive context switching
        "event_loop_blocking": 1,       # < 1ms blocking operations
        "coroutine_overhead": 0.1       # < 0.1ms per coroutine
    }
}
```

## Functional Validation

### 1. Channel Publishing Verification

#### 1.1 Per-Symbol Channel Routing
```python
FUNCTIONAL_TESTS = {
    "channel_routing": {
        "correct_channel_format": "market:{symbol}",
        "message_isolation": "100%",      # No cross-symbol contamination
        "channel_creation": "dynamic",    # Channels created on demand
        "channel_cleanup": "automatic"    # Unused channels cleaned up
    },
    "message_delivery": {
        "delivery_guarantee": "at_least_once",
        "message_ordering": "per_symbol",
        "duplicate_detection": "enabled",
        "message_format": "json_compatible"
    }
}
```

#### 1.2 Backward Compatibility
```python
COMPATIBILITY_TESTS = {
    "legacy_channel_support": {
        "dual_publishing": True,      # Publish to both old and new
        "configuration_driven": True, # Feature flag controlled
        "graceful_degradation": True, # Fallback to legacy on error
        "migration_path": "phased"    # Gradual migration support
    },
    "rust_consumer_compatibility": {
        "message_format": "unchanged",    # Same JSON structure
        "timestamp_format": "integer",    # Unix timestamp compatibility
        "error_handling": "graceful",     # No consumer failures
        "subscription_model": "adaptable" # Support both patterns
    }
}
```

### 2. Error Handling & Resilience

#### 2.1 Failure Scenarios
```python
ERROR_HANDLING_TESTS = {
    "redis_connection_failure": {
        "reconnect_attempts": 3,
        "backoff_strategy": "exponential",
        "fallback_behavior": "queue_locally",
        "recovery_time": 30  # seconds
    },
    "channel_publish_failure": {
        "retry_mechanism": "enabled",
        "max_retries": 3,
        "fallback_channel": "market:updates",
        "error_logging": "detailed"
    },
    "high_load_scenarios": {
        "backpressure_handling": "enabled",
        "queue_overflow": "configurable",
        "circuit_breaker": "per_channel",
        "graceful_degradation": "automatic"
    }
}
```

#### 2.2 Monitoring & Alerting
```python
MONITORING_REQUIREMENTS = {
    "metrics_collection": {
        "publish_success_rate": "> 99.9%",
        "channel_utilization": "per_symbol",
        "error_rate_threshold": "< 0.1%",
        "latency_percentiles": "[50, 95, 99]"
    },
    "health_checks": {
        "redis_connectivity": "continuous",
        "channel_availability": "per_symbol", 
        "message_flow_rate": "monitored",
        "resource_utilization": "tracked"
    }
}
```

## Integration Validation

### 1. End-to-End Flow Validation

#### 1.1 Data Pipeline Integrity
```python
PIPELINE_TESTS = {
    "data_ingestion_to_redis": {
        "websocket_to_channel": "< 100ms",
        "validation_overhead": "< 10ms",
        "storage_persistence": "parallel",
        "metrics_update": "real_time"
    },
    "multi_provider_support": {
        "polygon_integration": "verified",
        "alpaca_integration": "verified",
        "provider_isolation": "maintained",
        "failover_capability": "tested"
    }
}
```

#### 1.2 Consumer Integration
```python
CONSUMER_TESTS = {
    "rust_main_consumer": {
        "subscription_migration": "successful",
        "message_processing": "unchanged",
        "performance_impact": "< 5%",
        "error_handling": "robust"
    },
    "python_subscribers": {
        "redis_store_compatibility": "maintained",
        "subscription_patterns": "flexible",
        "channel_discovery": "dynamic",
        "backwards_compatibility": "100%"
    }
}
```

## Migration Success Milestones

### 1. Phase 2a: Dual Publishing Implementation

#### Milestone 1.1: Infrastructure Ready
- [ ] Configuration system supports feature flags
- [ ] ChannelManager class implemented and tested
- [ ] Enhanced metrics for per-symbol channels
- [ ] Redis connection pool optimizations

#### Milestone 1.2: Core Implementation
- [ ] Modified realtime_coordinator.py line 249
- [ ] Dual publishing (both legacy and per-symbol channels)
- [ ] Error handling and fallback mechanisms
- [ ] Unit tests passing (> 95% coverage)

#### Milestone 1.3: Integration Testing
- [ ] Redis integration tests passing
- [ ] Performance benchmarks meet targets
- [ ] Rust consumer compatibility verified
- [ ] End-to-end data flow validated

### 2. Phase 2b: Production Deployment

#### Milestone 2.1: Deployment Readiness
- [ ] Configuration validated in staging environment
- [ ] Monitoring dashboards updated
- [ ] Alerting thresholds configured
- [ ] Rollback procedures documented

#### Milestone 2.2: Production Validation
- [ ] Dual publishing active in production
- [ ] Performance metrics within targets
- [ ] Error rates below thresholds
- [ ] Consumer migration preparation complete

### 3. Phase 2c: Legacy Channel Retirement

#### Milestone 3.1: Consumer Migration Complete
- [ ] Rust consumer migrated to per-symbol channels
- [ ] Legacy channel usage at 0%
- [ ] Performance maintained or improved
- [ ] No consumer service disruptions

#### Milestone 3.2: Cleanup Complete
- [ ] Legacy channel publishing disabled
- [ ] Configuration cleanup completed
- [ ] Documentation updated
- [ ] Performance validation final

## Measurement & Validation Tools

### 1. Performance Monitoring

#### 1.1 Metrics Collection
```python
# Prometheus metrics for validation
METRICS_TO_TRACK = [
    "redis_publish_duration_seconds",
    "redis_publish_total",
    "redis_publish_errors_total", 
    "channel_active_count",
    "symbol_message_rate",
    "memory_usage_bytes",
    "cpu_usage_percent",
    "connection_pool_utilization"
]
```

#### 1.2 Performance Testing Scripts
```bash
#!/bin/bash
# Performance validation script

# Run load tests
python -m pytest tests/test_performance.py -m performance -v

# Measure baseline performance
python scripts/measure_baseline_performance.py

# Run channel isolation tests  
python scripts/test_channel_isolation.py --symbols 100 --duration 300

# Generate performance report
python scripts/generate_performance_report.py
```

### 2. Functional Validation

#### 2.1 Channel Verification
```python
async def validate_channel_routing():
    """Validate per-symbol channel routing."""
    test_symbols = ["AAPL", "NVDA", "TSLA", "MSFT", "GOOGL"]
    
    for symbol in test_symbols:
        # Publish test message
        await publish_test_message(symbol)
        
        # Verify channel isolation
        received = await verify_channel_isolation(symbol)
        assert received == 1, f"Channel isolation failed for {symbol}"
        
        # Verify message format
        message = await get_last_message(f"market:{symbol}")
        assert validate_message_format(message), f"Invalid format for {symbol}"
```

#### 2.2 Integration Testing
```bash
#!/bin/bash
# Integration test suite

# Test Redis integration
python -m pytest tests/test_redis_integration.py -m integration

# Test backward compatibility
python -m pytest tests/test_backward_compatibility.py

# Test end-to-end flow
python -m pytest tests/test_realtime_coordinator_integration.py

# Validate consumer compatibility
python scripts/validate_rust_consumer_compatibility.py
```

## Success Validation Report Template

### Final Validation Checklist
```markdown
## Phase 2 Data-Ingestion Migration Validation Report

### Performance Metrics ✅/❌
- [ ] Publishing latency < 2ms (95th percentile)
- [ ] Throughput > 50k msg/sec (100 symbols)
- [ ] Memory increase < 50MB
- [ ] CPU overhead < 5%

### Functional Requirements ✅/❌  
- [ ] Per-symbol channels working correctly
- [ ] Message isolation 100% effective
- [ ] Backward compatibility maintained
- [ ] Error handling robust

### Integration Validation ✅/❌
- [ ] Rust consumer compatibility verified
- [ ] End-to-end data flow intact
- [ ] Multi-provider support maintained
- [ ] Monitoring and alerting operational

### Migration Milestones ✅/❌
- [ ] Phase 2a: Dual publishing implemented
- [ ] Phase 2b: Production deployment successful  
- [ ] Phase 2c: Legacy channel retirement complete

### Risk Assessment
- **High Risk Items**: [List any high-risk findings]
- **Medium Risk Items**: [List medium-risk findings]
- **Recommendations**: [List recommendations for production]

### Go/No-Go Decision
**Recommendation**: ✅ GO / ❌ NO-GO
**Justification**: [Detailed reasoning for recommendation]
```

This comprehensive success criteria framework ensures that the migration is thoroughly validated from performance, functional, and integration perspectives before being considered complete.