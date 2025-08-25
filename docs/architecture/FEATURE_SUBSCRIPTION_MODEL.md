# Feature Subscription Model in Neural Trader

## The Key Question
**Do domain strategies need to subscribe to features, or are they automatically available?**

## Answer: It Depends on the Architecture Choice

Based on the codebase analysis, the system appears to be in transition. Here are the two models and what's actually implemented:

## Model 1: PUSH Model (Subscription-Based) 📡
*Features are pushed to subscribers via Redis Streams*

```
ML-Ops Platform (Produces Features)
    ↓ publishes to
Redis Stream: "stream:ml:features"
    ↓ subscribes
Trading Strategy (Consumes Features)
```

### How It Would Work:
```rust
// Trading strategy subscribes to feature stream
async fn subscribe_to_features() {
    let mut consumer = redis.subscribe("stream:ml:features").await?;
    
    while let Some(feature_msg) = consumer.next().await {
        // Process incoming features
        let features = parse_features(feature_msg);
        make_trading_decision(features);
    }
}
```

**Pros:**
- Real-time feature delivery
- Strategies only get features they need
- Decoupled architecture

**Cons:**
- Strategies must manage subscriptions
- Network overhead for streaming
- Complexity in managing consumer groups

## Model 2: PULL Model (Request-Based) 🎣
*Strategies request features when needed*

```
Trading Strategy: "I need features for AAPL"
    ↓ requests
ML-Ops Platform: "Here are the latest features"
    ↓ returns
Trading Strategy: Makes decision
```

### How It Would Work:
```rust
// Strategy requests features on-demand
async fn get_features_for_decision(symbol: &str) {
    let features = ml_ops_client.get_features(symbol).await?;
    make_trading_decision(features);
}
```

**Pros:**
- Simple to implement
- No subscription management
- Features always fresh when requested

**Cons:**
- Latency on each request
- Repeated computation if multiple strategies need same features
- Tighter coupling

## Model 3: HYBRID Model (Cache + Events) 🔄
*Features are broadcast and cached*

```
ML-Ops Platform
    ↓ broadcasts features
Redis Cache + Stream
    ↑ read from cache
Trading Strategies
```

### How It Works:
```rust
// Features are published to both cache and stream
impl FeaturePublisher {
    async fn publish_features(&self, features: Features) {
        // Store in cache for pull access
        redis.set(format!("features:{}", features.symbol), features).await?;
        
        // Also publish to stream for real-time subscribers
        redis.publish("stream:features", features).await?;
    }
}

// Strategies can choose how to consume
impl TradingStrategy {
    async fn get_features(&self, symbol: &str) -> Features {
        // Option 1: Get from cache (pull)
        if let Some(cached) = redis.get(format!("features:{}", symbol)).await? {
            return cached;
        }
        
        // Option 2: Already subscribed (push)
        if let Some(streamed) = self.feature_subscription.latest() {
            return streamed;
        }
        
        // Option 3: Request computation
        ml_ops_client.compute_features(symbol).await?
    }
}
```

## What's Actually Implemented in the Current Code? 🔍

Based on the codebase examination:

### Current State: INCOMPLETE IMPLEMENTATION
```rust
// neural-trading/src/events/consumer.rs
pub struct EventConsumer;  // Stub - not implemented

// neural-ml-ops - no Redis publishing found
// Features are calculated but not distributed
```

**The system is designed for Model 1 (Push/Subscribe) but not fully implemented:**

1. **Redis Streams channels are specified** in documentation
2. **EventConsumer exists** but is just a stub
3. **Feature publishing is missing** from neural-ml-ops
4. **No subscription logic** in trading strategies

## Recommended Approach 💡

### For Simplicity: Start with PULL Model
```rust
// neural-core/src/features/client.rs
pub struct FeatureClient {
    ml_ops_endpoint: String,
}

impl FeatureClient {
    pub async fn get_features(&self, request: FeatureRequest) -> Result<Features> {
        // Simple HTTP/gRPC call to ML-Ops
        self.client.get(format!("{}/features", self.ml_ops_endpoint))
            .json(&request)
            .send()
            .await?
            .json()
            .await
    }
}

// Usage in trading strategy
let features = feature_client.get_features(
    FeatureRequest::for_symbol("AAPL")
).await?;
```

### For Performance: Add Caching Layer
```rust
// Cache recent features to avoid repeated calls
pub struct CachedFeatureClient {
    client: FeatureClient,
    cache: HashMap<String, (Features, Instant)>,
    ttl: Duration,
}

impl CachedFeatureClient {
    pub async fn get_features(&mut self, symbol: &str) -> Result<Features> {
        // Check cache first
        if let Some((features, cached_at)) = self.cache.get(symbol) {
            if cached_at.elapsed() < self.ttl {
                return Ok(features.clone());
            }
        }
        
        // Fetch fresh features
        let features = self.client.get_features(symbol).await?;
        self.cache.insert(symbol.to_string(), (features.clone(), Instant::now()));
        Ok(features)
    }
}
```

### For Scale: Implement Pub/Sub Later
Only add subscription model when you need:
- Multiple strategies using same features
- Real-time feature updates
- Decoupled architecture for microservices

## The Practical Answer 🎯

**For Phase 3 (Current):**
- Features are NOT automatically available
- Strategies should PULL features when needed
- Use simple client/server model

**For Phase 4 (Future):**
- Add Redis Streams subscription
- Implement feature broadcasting
- Allow both push and pull models

## Decision Tree for Your Strategy

```
Do you need features?
├── YES → How often?
│   ├── Once per trade → PULL model (request when needed)
│   ├── Continuously → SUBSCRIBE model (Redis Streams)
│   └── Periodically → CACHE + PULL model
└── NO → Don't implement feature client

Is latency critical?
├── YES (< 10ms) → Cache locally or subscribe
└── NO (> 100ms ok) → Pull on demand

Multiple strategies need same features?
├── YES → Implement pub/sub to avoid recomputation
└── NO → Simple pull is fine
```

## Implementation Priority

1. **Start Simple**: Pull model with HTTP/gRPC
2. **Add Caching**: Reduce repeated calls
3. **Add Pub/Sub**: Only when scaling requires it

The current codebase has the infrastructure planned (Redis Streams) but not implemented. Start with pull model for simplicity!