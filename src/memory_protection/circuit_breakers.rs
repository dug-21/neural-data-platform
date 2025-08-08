//! Circuit breakers for memory protection
//!
//! Implements circuit breaker patterns to protect against memory exhaustion
//! by temporarily stopping operations when memory usage becomes critical.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};

/// Circuit breaker state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitBreakerState {
    Closed,    // Normal operation
    Open,      // Blocking operations
    HalfOpen,  // Testing if operations can resume
}

/// Circuit breaker for memory-sensitive operations
#[derive(Debug, Clone)]
pub struct MemoryCircuitBreaker {
    name: String,
    state: Arc<RwLock<CircuitBreakerState>>,
    failure_count: Arc<AtomicU32>,
    success_count: Arc<AtomicU32>,
    last_failure_time: Arc<AtomicU64>,
    last_success_time: Arc<AtomicU64>,
    failure_threshold: u32,
    timeout: Duration,
    half_open_max_calls: u32,
    half_open_calls: Arc<AtomicU32>,
}

impl MemoryCircuitBreaker {
    pub fn new(name: String, failure_threshold: u32, timeout: Duration) -> Self {
        Self {
            name,
            state: Arc::new(RwLock::new(CircuitBreakerState::Closed)),
            failure_count: Arc::new(AtomicU32::new(0)),
            success_count: Arc::new(AtomicU32::new(0)),
            last_failure_time: Arc::new(AtomicU64::new(0)),
            last_success_time: Arc::new(AtomicU64::new(0)),
            failure_threshold,
            timeout,
            half_open_max_calls: 5, // Allow 5 test calls in half-open state
            half_open_calls: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Execute a memory-sensitive operation through the circuit breaker
    pub async fn execute<F, T, E>(&self, operation: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Debug,
    {
        // Check if circuit breaker allows execution
        if !self.can_execute().await {
            return Err(CircuitBreakerError::CircuitOpen {
                circuit_name: self.name.clone(),
                state: self.get_state().await,
            });
        }

        // Execute the operation
        match operation.await {
            Ok(result) => {
                self.record_success().await;
                Ok(result)
            }
            Err(error) => {
                self.record_failure().await;
                Err(CircuitBreakerError::OperationFailed(error))
            }
        }
    }

    /// Check if the circuit breaker allows execution
    pub async fn can_execute(&self) -> bool {
        let state = self.get_state().await;
        match state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                // Check if timeout has elapsed and we should try half-open
                let now = Utc::now().timestamp() as u64;
                let last_failure = self.last_failure_time.load(Ordering::Relaxed);
                let timeout_elapsed = now - last_failure >= self.timeout.as_secs();

                if timeout_elapsed {
                    self.transition_to_half_open().await;
                    true
                } else {
                    false
                }
            }
            CircuitBreakerState::HalfOpen => {
                // Allow limited number of calls in half-open state
                let current_calls = self.half_open_calls.load(Ordering::Relaxed);
                current_calls < self.half_open_max_calls
            }
        }
    }

    /// Get current circuit breaker state
    pub async fn get_state(&self) -> CircuitBreakerState {
        *self.state.read().await
    }

    /// Check if circuit breaker is open (blocking operations)
    pub async fn is_open(&self) -> bool {
        matches!(self.get_state().await, CircuitBreakerState::Open)
    }

    /// Force the circuit breaker to open state
    pub async fn trip(&mut self) {
        warn!("Circuit breaker '{}' manually tripped", self.name);
        self.transition_to_open().await;
    }

    /// Force the circuit breaker to closed state
    pub async fn reset(&mut self) {
        info!("Circuit breaker '{}' manually reset", self.name);
        self.transition_to_closed().await;
    }

    /// Get circuit breaker statistics
    pub async fn get_stats(&self) -> CircuitBreakerStats {
        CircuitBreakerStats {
            name: self.name.clone(),
            state: self.get_state().await,
            failure_count: self.failure_count.load(Ordering::Relaxed),
            success_count: self.success_count.load(Ordering::Relaxed),
            failure_threshold: self.failure_threshold,
            timeout_seconds: self.timeout.as_secs(),
            last_failure_time: self.last_failure_time.load(Ordering::Relaxed),
            last_success_time: self.last_success_time.load(Ordering::Relaxed),
            half_open_calls: self.half_open_calls.load(Ordering::Relaxed),
        }
    }

    /// Record a successful operation
    async fn record_success(&self) {
        let now = Utc::now().timestamp() as u64;
        self.success_count.fetch_add(1, Ordering::Relaxed);
        self.last_success_time.store(now, Ordering::Relaxed);

        let state = self.get_state().await;
        match state {
            CircuitBreakerState::HalfOpen => {
                // After successful call in half-open, consider closing
                let half_open_calls = self.half_open_calls.fetch_add(1, Ordering::Relaxed);
                if half_open_calls >= self.half_open_max_calls / 2 {
                    info!("Circuit breaker '{}' closing after successful test", self.name);
                    self.transition_to_closed().await;
                }
            }
            CircuitBreakerState::Closed => {
                // Reset failure count on success in closed state
                self.failure_count.store(0, Ordering::Relaxed);
            }
            _ => {}
        }

        debug!("Circuit breaker '{}' recorded success", self.name);
    }

    /// Record a failed operation
    async fn record_failure(&self) {
        let now = Utc::now().timestamp() as u64;
        let failure_count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        self.last_failure_time.store(now, Ordering::Relaxed);

        let state = self.get_state().await;
        match state {
            CircuitBreakerState::Closed => {
                if failure_count >= self.failure_threshold {
                    warn!(
                        "Circuit breaker '{}' opening due to {} failures (threshold: {})",
                        self.name, failure_count, self.failure_threshold
                    );
                    self.transition_to_open().await;
                }
            }
            CircuitBreakerState::HalfOpen => {
                warn!("Circuit breaker '{}' opening due to failure in half-open state", self.name);
                self.transition_to_open().await;
            }
            _ => {}
        }

        debug!("Circuit breaker '{}' recorded failure (count: {})", self.name, failure_count);
    }

    /// Transition to open state
    async fn transition_to_open(&self) {
        let mut state = self.state.write().await;
        *state = CircuitBreakerState::Open;
        debug!("Circuit breaker '{}' transitioned to OPEN", self.name);
    }

    /// Transition to half-open state
    async fn transition_to_half_open(&self) {
        let mut state = self.state.write().await;
        *state = CircuitBreakerState::HalfOpen;
        self.half_open_calls.store(0, Ordering::Relaxed);
        debug!("Circuit breaker '{}' transitioned to HALF-OPEN", self.name);
    }

    /// Transition to closed state
    async fn transition_to_closed(&self) {
        let mut state = self.state.write().await;
        *state = CircuitBreakerState::Closed;
        self.failure_count.store(0, Ordering::Relaxed);
        self.half_open_calls.store(0, Ordering::Relaxed);
        debug!("Circuit breaker '{}' transitioned to CLOSED", self.name);
    }
}

/// Circuit breaker statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerStats {
    pub name: String,
    pub state: CircuitBreakerState,
    pub failure_count: u32,
    pub success_count: u32,
    pub failure_threshold: u32,
    pub timeout_seconds: u64,
    pub last_failure_time: u64,
    pub last_success_time: u64,
    pub half_open_calls: u32,
}

/// Circuit breaker errors
#[derive(Debug)]
pub enum CircuitBreakerError<E> {
    CircuitOpen {
        circuit_name: String,
        state: CircuitBreakerState,
    },
    OperationFailed(E),
}

impl<E: std::fmt::Display> std::fmt::Display for CircuitBreakerError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitBreakerError::CircuitOpen { circuit_name, state } => {
                write!(f, "Circuit breaker '{}' is in {:?} state", circuit_name, state)
            }
            CircuitBreakerError::OperationFailed(e) => {
                write!(f, "Operation failed: {}", e)
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for CircuitBreakerError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CircuitBreakerError::OperationFailed(e) => Some(e),
            _ => None,
        }
    }
}

/// Memory-aware circuit breaker that monitors memory usage
#[derive(Debug)]
pub struct MemoryAwareCircuitBreaker {
    base_breaker: MemoryCircuitBreaker,
    memory_threshold_bytes: u64,
    current_memory_usage: Arc<AtomicU64>,
}

impl MemoryAwareCircuitBreaker {
    pub fn new(
        name: String,
        failure_threshold: u32,
        timeout: Duration,
        memory_threshold_bytes: u64,
    ) -> Self {
        Self {
            base_breaker: MemoryCircuitBreaker::new(name, failure_threshold, timeout),
            memory_threshold_bytes,
            current_memory_usage: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Update current memory usage
    pub fn update_memory_usage(&self, usage_bytes: u64) {
        self.current_memory_usage.store(usage_bytes, Ordering::Relaxed);
    }

    /// Execute operation with memory usage check
    pub async fn execute<F, T, E>(&self, operation: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Debug,
    {
        // Check memory usage before executing
        let current_memory = self.current_memory_usage.load(Ordering::Relaxed);
        if current_memory > self.memory_threshold_bytes {
            warn!(
                "Memory usage ({} MB) exceeds threshold ({} MB), circuit breaker blocking operation",
                current_memory / 1024 / 1024,
                self.memory_threshold_bytes / 1024 / 1024
            );
            return Err(CircuitBreakerError::CircuitOpen {
                circuit_name: self.base_breaker.name.clone(),
                state: CircuitBreakerState::Open,
            });
        }

        // Execute through base circuit breaker
        self.base_breaker.execute(operation).await
    }

    /// Get statistics including memory information
    pub async fn get_stats(&self) -> MemoryAwareCircuitBreakerStats {
        let base_stats = self.base_breaker.get_stats().await;
        MemoryAwareCircuitBreakerStats {
            base_stats,
            memory_threshold_bytes: self.memory_threshold_bytes,
            current_memory_usage_bytes: self.current_memory_usage.load(Ordering::Relaxed),
            memory_usage_percent: (self.current_memory_usage.load(Ordering::Relaxed) as f64
                / self.memory_threshold_bytes as f64)
                * 100.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAwareCircuitBreakerStats {
    #[serde(flatten)]
    pub base_stats: CircuitBreakerStats,
    pub memory_threshold_bytes: u64,
    pub current_memory_usage_bytes: u64,
    pub memory_usage_percent: f64,
}

/// Circuit breaker manager for coordinating multiple circuit breakers
#[derive(Debug)]
pub struct CircuitBreakerManager {
    breakers: Arc<RwLock<std::collections::HashMap<String, MemoryAwareCircuitBreaker>>>,
}

impl CircuitBreakerManager {
    pub fn new() -> Self {
        Self {
            breakers: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Register a new circuit breaker
    pub async fn register_breaker(
        &self,
        name: String,
        failure_threshold: u32,
        timeout: Duration,
        memory_threshold_bytes: u64,
    ) {
        let breaker = MemoryAwareCircuitBreaker::new(
            name.clone(),
            failure_threshold,
            timeout,
            memory_threshold_bytes,
        );
        
        let mut breakers = self.breakers.write().await;
        breakers.insert(name.clone(), breaker);
        info!("Registered circuit breaker: {}", name);
    }

    /// Get a circuit breaker by name
    pub async fn get_breaker(&self, name: &str) -> Option<MemoryAwareCircuitBreaker> {
        let breakers = self.breakers.read().await;
        breakers.get(name).cloned()
    }

    /// Update memory usage for all circuit breakers
    pub async fn update_memory_usage(&self, usage_bytes: u64) {
        let breakers = self.breakers.read().await;
        for breaker in breakers.values() {
            breaker.update_memory_usage(usage_bytes);
        }
    }

    /// Get statistics for all circuit breakers
    pub async fn get_all_stats(&self) -> std::collections::HashMap<String, MemoryAwareCircuitBreakerStats> {
        let breakers = self.breakers.read().await;
        let mut stats = std::collections::HashMap::new();
        
        for (name, breaker) in breakers.iter() {
            stats.insert(name.clone(), breaker.get_stats().await);
        }
        
        stats
    }

    /// Check if any circuit breakers are open
    pub async fn has_open_breakers(&self) -> bool {
        let breakers = self.breakers.read().await;
        for breaker in breakers.values() {
            let stats = breaker.get_stats().await;
            if matches!(stats.base_stats.state, CircuitBreakerState::Open) {
                return true;
            }
        }
        false
    }
}

impl Default for CircuitBreakerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use tokio::time::{sleep, Duration as TokioDuration};

    #[tokio::test]
    async fn test_circuit_breaker_closed_state() {
        let breaker = MemoryCircuitBreaker::new(
            "test_breaker".to_string(),
            3,
            Duration::from_secs(5),
        );

        // Should be closed initially
        assert_eq!(breaker.get_state().await, CircuitBreakerState::Closed);
        assert!(breaker.can_execute().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens_on_failures() {
        let breaker = MemoryCircuitBreaker::new(
            "test_breaker".to_string(),
            2, // Low threshold for testing
            Duration::from_secs(1),
        );

        // First failure
        let result = breaker.execute(async { Err::<(), &str>("failure1") }).await;
        assert!(result.is_err());
        assert_eq!(breaker.get_state().await, CircuitBreakerState::Closed);

        // Second failure should open the circuit
        let result = breaker.execute(async { Err::<(), &str>("failure2") }).await;
        assert!(result.is_err());
        assert_eq!(breaker.get_state().await, CircuitBreakerState::Open);

        // Should not allow execution when open
        assert!(!breaker.can_execute().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_transition() {
        let breaker = MemoryCircuitBreaker::new(
            "test_breaker".to_string(),
            1, // Very low threshold
            Duration::from_millis(100), // Short timeout
        );

        // Trigger failure to open circuit
        let _ = breaker.execute(async { Err::<(), &str>("failure") }).await;
        assert_eq!(breaker.get_state().await, CircuitBreakerState::Open);

        // Wait for timeout
        sleep(TokioDuration::from_millis(150)).await;

        // Should transition to half-open
        assert!(breaker.can_execute().await);
        
        // Execute a successful operation to check half-open behavior
        let result = breaker.execute(async { Ok::<&str, &str>("success") }).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_memory_aware_circuit_breaker() {
        let breaker = MemoryAwareCircuitBreaker::new(
            "memory_test".to_string(),
            5,
            Duration::from_secs(5),
            1024 * 1024, // 1MB threshold
        );

        // Should work with low memory usage
        breaker.update_memory_usage(512 * 1024); // 512KB
        let result = breaker.execute(async { Ok::<&str, &str>("success") }).await;
        assert!(result.is_ok());

        // Should block with high memory usage
        breaker.update_memory_usage(2 * 1024 * 1024); // 2MB
        let result = breaker.execute(async { Ok::<&str, &str>("should_fail") }).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_circuit_breaker_manager() {
        let manager = CircuitBreakerManager::new();

        // Register a breaker
        manager.register_breaker(
            "test_manager".to_string(),
            3,
            Duration::from_secs(5),
            1024 * 1024,
        ).await;

        // Should be able to get the breaker
        let breaker = manager.get_breaker("test_manager").await;
        assert!(breaker.is_some());

        // Update memory usage
        manager.update_memory_usage(512 * 1024).await;

        // Get stats
        let stats = manager.get_all_stats().await;
        assert_eq!(stats.len(), 1);
        assert!(stats.contains_key("test_manager"));
    }
}