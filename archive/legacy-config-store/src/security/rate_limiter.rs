use crate::ConfigError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
}

pub struct RateLimiter {
    buckets: Arc<Mutex<HashMap<String, TokenBucket>>>,
    max_tokens: f64,
    refill_duration: Duration,
}

impl std::fmt::Debug for RateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RateLimiter")
            .field("max_tokens", &self.max_tokens)
            .field("refill_duration", &self.refill_duration)
            .finish()
    }
}

impl RateLimiter {
    pub fn new(max_requests: u32, window: Duration) -> Self {
        Self {
            buckets: Arc::new(Mutex::new(HashMap::new())),
            max_tokens: max_requests as f64,
            refill_duration: window,
        }
    }

    pub fn check(&self, client_id: &str) -> Result<(), ConfigError> {
        let mut buckets = self.buckets.lock().unwrap();
        let now = Instant::now();

        let bucket = buckets
            .entry(client_id.to_string())
            .or_insert_with(|| TokenBucket {
                tokens: self.max_tokens,
                last_refill: now,
            });

        // Calculate tokens to add based on time elapsed
        let elapsed = now.duration_since(bucket.last_refill);
        let refill_rate = self.max_tokens / self.refill_duration.as_secs_f64();
        let tokens_to_add = elapsed.as_secs_f64() * refill_rate;

        // Refill tokens
        bucket.tokens = (bucket.tokens + tokens_to_add).min(self.max_tokens);
        bucket.last_refill = now;

        // Check if we have tokens available
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            Ok(())
        } else {
            Err(ConfigError::RateLimitExceeded)
        }
    }

    pub fn reset(&self, client_id: &str) {
        let mut buckets = self.buckets.lock().unwrap();
        buckets.remove(client_id);
    }
}
