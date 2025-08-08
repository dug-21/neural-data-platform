# INTERFACE CONTRACT - Phase 2 Channel Architecture
## 🚨 CRITICAL: Python ↔ Rust Service Compatibility Specification

**Document Type**: Interface Contract & Compatibility Guarantee  
**Priority**: CRITICAL - Zero-Tolerance Compatibility Requirements  
**Teams**: Data-Ingestion (Python) ↔ Neural-Trader (Rust)  
**Created**: 2025-08-08  
**Status**: BINDING CONTRACT - Must be followed exactly  

---

## 📋 Executive Summary

This contract defines the **EXACT** interface specifications that both Python (data-ingestion) and Rust (neural-trader) services MUST implement to ensure seamless channel-based communication. Any deviation from these specifications will result in system incompatibility.

**NON-NEGOTIABLE REQUIREMENTS:**
- ✅ Identical channel naming: `market:{symbol}`
- ✅ Identical message JSON schema (field-for-field)
- ✅ Synchronized migration timeline
- ✅ Compatible error handling protocols

---

## 🎯 Channel Naming Convention - MANDATORY

### Standard Format (BOTH SERVICES)
```
PATTERN: market:{SYMBOL}
```

### Required Channel Examples (BOTH SERVICES MUST SUPPORT)
```
market:AAPL    - Apple Inc. market data
market:MSFT    - Microsoft Corporation market data  
market:GOOGL   - Alphabet Inc. market data
market:NVDA    - NVIDIA Corporation market data
market:TSLA    - Tesla Inc. market data
market:META    - Meta Platforms Inc. market data
market:AMZN    - Amazon.com Inc. market data
market:JPM     - JPMorgan Chase & Co. market data
market:BAC     - Bank of America Corp. market data
market:XOM     - Exxon Mobil Corporation market data
```

### ⚠️ CRITICAL REQUIREMENTS

**Symbol Normalization (BOTH SERVICES):**
- Symbols MUST be uppercase: `AAPL`, `MSFT`, `GOOGL`
- NO lowercase allowed: ~~`aapl`~~, ~~`msft`~~, ~~`googl`~~
- NO special characters: ~~`AAPL.US`~~, ~~`MSFT-USD`~~

**Channel Validation (BOTH SERVICES):**
```rust
// Rust implementation
fn validate_channel_name(channel: &str) -> bool {
    let pattern = regex::Regex::new(r"^market:[A-Z]{1,5}$").unwrap();
    pattern.is_match(channel)
}
```

```python
# Python implementation  
import re

def validate_channel_name(channel: str) -> bool:
    pattern = re.compile(r"^market:[A-Z]{1,5}$")
    return pattern.match(channel) is not None
```

---

## 💬 Message Schema - EXACT COMPATIBILITY REQUIRED

### Standard MarketData JSON Format (BOTH SERVICES)

```json
{
  "symbol": "NVDA",
  "timestamp": "2025-08-08T15:30:00.000Z",
  "price": 445.67,
  "volume": 1500,
  "bid": 445.60,
  "ask": 445.70,
  "spread": 0.10,
  "market_session": "regular",
  "sequence_number": 12345,
  "quality_score": 0.98,
  "source": "polygon",
  "metadata": {
    "open": 440.50,
    "high": 446.00,
    "low": 439.80,
    "close": 445.67,
    "market_cap": 1200000000000,
    "sector": "technology"
  }
}
```

### MANDATORY Field Specifications

#### Core Fields (REQUIRED - BOTH SERVICES)
| Field | Type | Format | Example | Validation |
|-------|------|---------|---------|------------|
| `symbol` | String | Uppercase, 1-5 chars | `"NVDA"` | `^[A-Z]{1,5}$` |
| `timestamp` | String | ISO 8601 UTC | `"2025-08-08T15:30:00.000Z"` | RFC 3339 |
| `price` | Number | Float64, positive | `445.67` | `> 0.0` |
| `volume` | Integer | Uint64, non-negative | `1500` | `>= 0` |
| `bid` | Number | Float64, positive | `445.60` | `> 0.0` |
| `ask` | Number | Float64, positive | `445.70` | `> 0.0` |

#### Extended Fields (REQUIRED - BOTH SERVICES)  
| Field | Type | Format | Example | Validation |
|-------|------|---------|---------|------------|
| `spread` | Number | Float64, non-negative | `0.10` | `>= 0.0` |
| `market_session` | String | Enum | `"regular"` | `regular\|pre\|after` |
| `sequence_number` | Integer | Uint64 | `12345` | Monotonic increase |
| `quality_score` | Number | Float64, 0.0-1.0 | `0.98` | `0.0 <= x <= 1.0` |
| `source` | String | Lowercase identifier | `"polygon"` | `^[a-z_]+$` |

#### Metadata Fields (OPTIONAL - BOTH SERVICES)
```json
"metadata": {
  "open": 440.50,           // Optional: Float64
  "high": 446.00,           // Optional: Float64  
  "low": 439.80,            // Optional: Float64
  "close": 445.67,          // Optional: Float64
  "market_cap": 1200000000000, // Optional: Uint64
  "sector": "technology"    // Optional: String
}
```

### Data Type Mapping

#### Python ↔ Rust Type Compatibility
| JSON Type | Python Type | Rust Type | Serde Requirement |
|-----------|-------------|-----------|-------------------|
| String | `str` | `String` | `#[serde(rename = "field")]` |
| Number (Float) | `float` | `f64` | Default |
| Number (Int) | `int` | `u64` | Default |
| Boolean | `bool` | `bool` | Default |
| Object | `dict` | `HashMap<String, Value>` | `#[serde(flatten)]` |
| Array | `list` | `Vec<T>` | Default |

---

## 🔄 Migration Strategy - SYNCHRONIZED EXECUTION

### Phase 2A: Dual Publishing (Duration: 2 Days)
**PYTHON TEAM ACTIONS:**
```python
async def publish_market_data(self, data: MarketData) -> None:
    # 1. CONTINUE publishing to legacy channel (backward compatibility)
    await self.redis.publish("market:updates", data.to_json())
    
    # 2. START publishing to symbol-specific channels
    symbol_channel = f"market:{data.symbol}"
    await self.redis.publish(symbol_channel, data.to_json())
    
    # 3. Log dual publishing for monitoring
    logger.info(f"Dual published {data.symbol} to both channels")
```

**RUST TEAM ACTIONS:**
```rust
async fn subscribe_to_market_data(&self) -> Result<()> {
    // 1. MAINTAIN existing legacy subscription
    let legacy_subscriber = self.redis.subscribe("market:updates").await?;
    
    // 2. ADD symbol-specific subscriptions
    let symbol_channels: Vec<String> = self.configured_symbols
        .iter()
        .map(|s| format!("market:{}", s))
        .collect();
    
    let symbol_subscriber = self.redis.subscribe(&symbol_channels).await?;
    
    // 3. Process from both sources (deduplication logic)
    tokio::spawn(self.process_dual_streams(legacy_subscriber, symbol_subscriber));
    Ok(())
}
```

### Phase 2B: Validation & Testing (Duration: 1 Day)
**COORDINATED TESTING ACTIVITIES:**

1. **Message Compatibility Test:**
   ```bash
   # Python publishes test message
   python -m data_ingestion.test_publisher --symbol AAPL --count 100
   
   # Rust verifies receipt and parsing
   cargo test test_symbol_channel_compatibility -- --nocapture
   ```

2. **Load Testing:**
   ```bash
   # Python: High-frequency publishing
   python -m data_ingestion.load_test --symbols NVDA,AAPL,MSFT --rate 1000
   
   # Rust: Parallel consumption validation
   cargo test test_multi_symbol_consumption_load -- --nocapture
   ```

3. **Error Handling Test:**
   ```bash
   # Python: Simulate Redis failures
   python -m data_ingestion.test_error_handling --failure-rate 0.1
   
   # Rust: Verify graceful degradation
   cargo test test_publisher_failure_handling -- --nocapture
   ```

### Phase 2C: Legacy Deprecation (Duration: 1 Day)
**COORDINATED CUTOVER:**

**Step 1: Rust switches to symbol-only mode (T+0)**
```rust
// Remove legacy channel subscription
// self.redis.unsubscribe("market:updates").await?;

// Keep only symbol-specific subscriptions
let symbol_channels: Vec<String> = self.configured_symbols
    .iter()
    .map(|s| format!("market:{}", s))
    .collect();
let subscriber = self.redis.subscribe(&symbol_channels).await?;
```

**Step 2: Python stops dual publishing (T+1 hour)**
```python
async def publish_market_data(self, data: MarketData) -> None:
    # REMOVE: await self.redis.publish("market:updates", data.to_json())
    
    # KEEP ONLY: Symbol-specific publishing
    symbol_channel = f"market:{data.symbol}"
    await self.redis.publish(symbol_channel, data.to_json())
```

**Step 3: Validation (T+2 hours)**
- Monitor message flow for 2 hours
- Verify zero message loss
- Confirm all symbols receiving equal processing

---

## ⚠️ Error Handling Protocol - IDENTICAL IMPLEMENTATION

### Circuit Breaker Configuration (BOTH SERVICES)
```yaml
# Shared configuration (both Python and Rust)
circuit_breaker:
  failure_threshold: 5          # Open after 5 consecutive failures
  recovery_timeout_seconds: 30  # Try recovery after 30 seconds
  half_open_max_calls: 3        # Allow 3 test calls in half-open state
```

**Python Implementation:**
```python
class CircuitBreaker:
    def __init__(self, failure_threshold=5, recovery_timeout=30):
        self.failure_threshold = failure_threshold
        self.recovery_timeout = recovery_timeout
        self.failure_count = 0
        self.last_failure_time = None
        self.state = "CLOSED"  # CLOSED, OPEN, HALF_OPEN
    
    async def publish_with_circuit_breaker(self, channel: str, data: str):
        if not self.allow_request(channel):
            raise CircuitBreakerOpenError(f"Circuit breaker open for {channel}")
        
        try:
            result = await self.redis.publish(channel, data)
            self.record_success(channel)
            return result
        except Exception as e:
            self.record_failure(channel)
            raise
```

**Rust Implementation:**
```rust
pub struct CircuitBreaker {
    failure_threshold: u32,
    recovery_timeout: Duration,
    channel_states: Arc<RwLock<HashMap<String, CircuitState>>>,
}

impl CircuitBreaker {
    pub async fn publish_with_circuit_breaker(
        &self,
        redis: &RedisClient,
        channel: &str,
        data: &str,
    ) -> Result<u64> {
        if !self.allow_request(channel).await {
            return Err(anyhow!("Circuit breaker open for channel: {}", channel));
        }
        
        match redis.publish(channel, data).await {
            Ok(result) => {
                self.record_success(channel).await;
                Ok(result)
            }
            Err(e) => {
                self.record_failure(channel).await;
                Err(e)
            }
        }
    }
}
```

### Retry Logic (BOTH SERVICES)
```yaml
# Shared retry configuration
retry:
  max_attempts: 3
  base_delay_ms: 100
  max_delay_ms: 5000
  backoff_multiplier: 2.0
```

---

## 📊 Performance Expectations - BINDING SLA

### Throughput Requirements (BOTH SERVICES MUST MEET)
- **Per-Symbol Channel**: 10,000+ messages/second sustained
- **Total System**: 100,000+ messages/second across all channels
- **Memory Usage**: <2GB for 50 symbols
- **CPU Usage**: <50% sustained during market hours

### Latency Requirements (BOTH SERVICES MUST MEET)
- **Publishing Latency**: <5ms average (Python)
- **Consumption Latency**: <10ms average (Rust)
- **End-to-End Latency**: <15ms (publish to process)

### Reliability Requirements (BOTH SERVICES MUST MEET)
- **Availability**: 99.9% uptime during market hours (9:30 AM - 4:00 PM EST)
- **Error Rate**: <0.1% message publishing/consumption failures
- **Recovery Time**: <30 seconds for Redis connection failures

---

## 🔍 Integration Testing Protocol

### Compatibility Test Suite
**Both teams MUST pass these tests:**

```bash
# Test 1: Channel Naming Validation
test_channel_naming_compatibility()

# Test 2: Message Schema Validation  
test_message_schema_compatibility()

# Test 3: Load Testing
test_multi_symbol_load_handling()

# Test 4: Error Handling
test_circuit_breaker_coordination()

# Test 5: Migration Process
test_dual_publishing_phase()

# Test 6: Performance Validation
test_latency_throughput_requirements()
```

### Continuous Compatibility Monitoring
```yaml
# monitoring/compatibility-tests.yml
compatibility_tests:
  interval: "5m"
  tests:
    - name: "message_format_validation"
      python_publisher: true
      rust_consumer: true
      success_threshold: 100%
    
    - name: "channel_naming_validation"
      symbol_list: ["AAPL", "MSFT", "GOOGL", "NVDA", "TSLA"]
      channel_format: "market:{symbol}"
      success_threshold: 100%
    
    - name: "performance_validation"
      throughput_min: 10000
      latency_max_ms: 15
      success_threshold: 99.9%
```

---

## ✅ Acceptance Criteria - MUST BE VERIFIED

### Phase 2 Completion Requirements
- [ ] **Channel Naming**: Both services use identical `market:{symbol}` format
- [ ] **Message Schema**: Both services send/receive identical JSON structure  
- [ ] **Migration**: Dual publishing → Validation → Legacy deprecation completed
- [ ] **Performance**: Both services meet throughput/latency SLAs
- [ ] **Error Handling**: Circuit breaker and retry logic behave identically
- [ ] **Testing**: All compatibility tests pass continuously

### Integration Validation Checklist
- [ ] **Python publishes to `market:AAPL`** → **Rust receives from `market:AAPL`** ✅
- [ ] **Python publishes to `market:NVDA`** → **Rust receives from `market:NVDA`** ✅  
- [ ] **JSON message schema identical** → **No parsing errors** ✅
- [ ] **High-frequency NVDA messages** → **No monopolization, fair processing** ✅
- [ ] **Redis failure simulation** → **Both services handle gracefully** ✅

---

## 🚨 Conflict Resolution Protocol

### Design Conflicts
If teams discover incompatible designs:
1. **Immediate escalation** to mesh coordinator
2. **Emergency alignment meeting** within 2 hours
3. **Binding resolution** with updated contract
4. **Re-validation** of affected components

### Implementation Conflicts  
If implementation details differ:
1. **Python team** implements exact Rust interface
2. **Rust team** validates Python compatibility
3. **Joint testing** until 100% compatibility achieved

---

## 📋 BINDING COMMITMENT

**Python Team Commitment:**
- [ ] Implement exact channel naming: `market:{symbol}`
- [ ] Use exact JSON schema as specified
- [ ] Follow synchronized migration timeline
- [ ] Meet all performance requirements

**Rust Team Commitment:**  
- [ ] Subscribe to exact channel naming: `market:{symbol}`
- [ ] Parse exact JSON schema as specified
- [ ] Follow synchronized migration timeline
- [ ] Meet all performance requirements

**Both Teams Commitment:**
- [ ] No deviations without explicit contract amendment
- [ ] Continuous integration testing
- [ ] Performance monitoring and alerting
- [ ] 24/7 support during migration phases

---

## 📞 Emergency Contacts

**Mesh Coordinator**: Available 24/7 during Phase 2 implementation
**Python Team Lead**: [Contact Info]
**Rust Team Lead**: [Contact Info]  
**DevOps/Infrastructure**: [Contact Info]

---

**This contract is BINDING and must be followed exactly to ensure system compatibility and prevent service disruption.**

**Signature Required**: Both team leads must acknowledge understanding and commitment to this interface contract.

---
*Document Version: 1.0*  
*Last Updated: 2025-08-08T01:53:06Z*  
*Next Review: After Phase 2 completion*