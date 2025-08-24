// Test Utilities - London School TDD Support
// Common utilities and helpers for orchestrator testing

use super::mock_services::*;
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use tokio::time::timeout;

/// Test execution utilities
pub struct TestExecutor {
    timeout_duration: Duration,
}

impl TestExecutor {
    pub fn new() -> Self {
        Self {
            timeout_duration: Duration::from_secs(30),
        }
    }
    
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout_duration = timeout;
        self
    }
    
    pub async fn execute_with_timeout<F, T>(&self, future: F) -> Result<T, String>
    where
        F: std::future::Future<Output = T>,
    {
        timeout(self.timeout_duration, future)
            .await
            .map_err(|_| format!("Test timed out after {:?}", self.timeout_duration))
    }
}

/// Mock service verification utilities
pub struct MockVerifier;

impl MockVerifier {
    /// Verify mock service interactions match expected patterns
    pub fn verify_interactions(
        actual_calls: &[SwarmCall],
        expected_patterns: &[&str],
    ) -> Result<(), String> {
        for pattern in expected_patterns {
            let matching_calls = actual_calls
                .iter()
                .filter(|call| call.method.contains(pattern))
                .count();
                
            if matching_calls == 0 {
                return Err(format!("Expected call pattern '{}' not found", pattern));
            }
        }
        Ok(())
    }
    
    /// Verify mock service call sequence
    pub fn verify_call_sequence(
        actual_calls: &[SwarmCall],
        expected_sequence: &[&str],
    ) -> Result<(), String> {
        if actual_calls.len() < expected_sequence.len() {
            return Err(format!(
                "Expected {} calls in sequence, but got {}",
                expected_sequence.len(),
                actual_calls.len()
            ));
        }
        
        for (i, expected_method) in expected_sequence.iter().enumerate() {
            if !actual_calls[i].method.contains(expected_method) {
                return Err(format!(
                    "Expected call {} to be '{}', but got '{}'",
                    i, expected_method, actual_calls[i].method
                ));
            }
        }
        
        Ok(())
    }
    
    /// Verify no unexpected side effects occurred
    pub fn verify_no_side_effects(
        actual_calls: &[SwarmCall],
        forbidden_patterns: &[&str],
    ) -> Result<(), String> {
        for pattern in forbidden_patterns {
            let matching_calls = actual_calls
                .iter()
                .filter(|call| call.method.contains(pattern))
                .collect::<Vec<_>>();
                
            if !matching_calls.is_empty() {
                return Err(format!(
                    "Forbidden call pattern '{}' found in calls: {:?}",
                    pattern, matching_calls
                ));
            }
        }
        Ok(())
    }
}

/// Performance measurement utilities
pub struct PerformanceMeasurement {
    start_time: SystemTime,
    measurements: HashMap<String, Duration>,
}

impl PerformanceMeasurement {
    pub fn start() -> Self {
        Self {
            start_time: SystemTime::now(),
            measurements: HashMap::new(),
        }
    }
    
    pub fn measure<F, T>(&mut self, operation_name: &str, operation: F) -> T
    where
        F: FnOnce() -> T,
    {
        let start = SystemTime::now();
        let result = operation();
        let duration = start.elapsed().unwrap_or_default();
        self.measurements.insert(operation_name.to_string(), duration);
        result
    }
    
    pub async fn measure_async<F, T>(&mut self, operation_name: &str, operation: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        let start = SystemTime::now();
        let result = operation.await;
        let duration = start.elapsed().unwrap_or_default();
        self.measurements.insert(operation_name.to_string(), duration);
        result
    }
    
    pub fn get_measurement(&self, operation_name: &str) -> Option<Duration> {
        self.measurements.get(operation_name).copied()
    }
    
    pub fn assert_performance_threshold(
        &self,
        operation_name: &str,
        max_duration: Duration,
    ) -> Result<(), String> {
        match self.get_measurement(operation_name) {
            Some(actual) => {
                if actual > max_duration {
                    Err(format!(
                        "Operation '{}' took {:?}, expected <= {:?}",
                        operation_name, actual, max_duration
                    ))
                } else {
                    Ok(())
                }
            }
            None => Err(format!("No measurement found for operation '{}'", operation_name)),
        }
    }
    
    pub fn get_total_duration(&self) -> Duration {
        self.start_time.elapsed().unwrap_or_default()
    }
}

/// Contract testing utilities for London School TDD
pub struct ContractVerifier;

impl ContractVerifier {
    /// Verify that mock interactions match expected contracts
    pub fn verify_mock_contract(
        mock_calls: &[SwarmCall],
        expected_contract: &MockContract,
    ) -> Result<(), String> {
        // Verify required method calls
        for required_method in &expected_contract.required_methods {
            let method_called = mock_calls
                .iter()
                .any(|call| call.method == *required_method);
                
            if !method_called {
                return Err(format!(
                    "Required method '{}' was not called",
                    required_method
                ));
            }
        }
        
        // Verify forbidden method calls
        for forbidden_method in &expected_contract.forbidden_methods {
            let method_called = mock_calls
                .iter()
                .any(|call| call.method == *forbidden_method);
                
            if method_called {
                return Err(format!(
                    "Forbidden method '{}' was called",
                    forbidden_method
                ));
            }
        }
        
        // Verify parameter contracts
        for (method, expected_params) in &expected_contract.method_parameters {
            let method_calls: Vec<_> = mock_calls
                .iter()
                .filter(|call| call.method == *method)
                .collect();
                
            for call in method_calls {
                Self::verify_parameters(&call.parameters, expected_params)?;
            }
        }
        
        Ok(())
    }
    
    fn verify_parameters(
        actual: &HashMap<String, String>,
        expected: &HashMap<String, String>,
    ) -> Result<(), String> {
        for (key, expected_value) in expected {
            match actual.get(key) {
                Some(actual_value) => {
                    if actual_value != expected_value {
                        return Err(format!(
                            "Parameter '{}' expected '{}', got '{}'",
                            key, expected_value, actual_value
                        ));
                    }
                }
                None => {
                    return Err(format!("Required parameter '{}' not found", key));
                }
            }
        }
        Ok(())
    }
}

/// Mock contract definition for London School TDD
pub struct MockContract {
    pub required_methods: Vec<String>,
    pub forbidden_methods: Vec<String>,
    pub method_parameters: HashMap<String, HashMap<String, String>>,
    pub expected_call_count: HashMap<String, usize>,
}

impl MockContract {
    pub fn new() -> Self {
        Self {
            required_methods: Vec::new(),
            forbidden_methods: Vec::new(),
            method_parameters: HashMap::new(),
            expected_call_count: HashMap::new(),
        }
    }
    
    pub fn require_method(mut self, method: &str) -> Self {
        self.required_methods.push(method.to_string());
        self
    }
    
    pub fn forbid_method(mut self, method: &str) -> Self {
        self.forbidden_methods.push(method.to_string());
        self
    }
    
    pub fn expect_parameters(
        mut self,
        method: &str,
        parameters: HashMap<String, String>,
    ) -> Self {
        self.method_parameters.insert(method.to_string(), parameters);
        self
    }
    
    pub fn expect_call_count(mut self, method: &str, count: usize) -> Self {
        self.expected_call_count.insert(method.to_string(), count);
        self
    }
}

/// Test data validation utilities
pub struct TestDataValidator;

impl TestDataValidator {
    /// Validate agent configuration
    pub fn validate_agent(agent: &MockAgent) -> Result<(), String> {
        if agent.id.is_empty() {
            return Err("Agent ID cannot be empty".to_string());
        }
        
        if agent.capabilities.is_empty() {
            return Err("Agent must have at least one capability".to_string());
        }
        
        if agent.cpu_usage > 1.0 {
            return Err(format!(
                "Agent CPU usage {} exceeds maximum of 1.0",
                agent.cpu_usage
            ));
        }
        
        if agent.memory_usage == 0 {
            return Err("Agent memory usage must be greater than 0".to_string());
        }
        
        Ok(())
    }
    
    /// Validate task configuration
    pub fn validate_task(task: &MockTask) -> Result<(), String> {
        if task.id.is_empty() {
            return Err("Task ID cannot be empty".to_string());
        }
        
        if task.description.is_empty() {
            return Err("Task description cannot be empty".to_string());
        }
        
        // Validate task status consistency
        match task.status {
            TaskStatus::Completed => {
                if task.completed_at.is_none() {
                    return Err("Completed task must have completion timestamp".to_string());
                }
                if task.result.is_none() {
                    return Err("Completed task should have result".to_string());
                }
            }
            TaskStatus::Failed => {
                if task.error.is_none() {
                    return Err("Failed task must have error message".to_string());
                }
            }
            TaskStatus::InProgress => {
                if task.started_at.is_none() {
                    return Err("In-progress task must have start timestamp".to_string());
                }
            }
            _ => {}
        }
        
        Ok(())
    }
    
    /// Validate swarm configuration
    pub fn validate_swarm(swarm: &SwarmInstance) -> Result<(), String> {
        if swarm.id.is_empty() {
            return Err("Swarm ID cannot be empty".to_string());
        }
        
        if swarm.max_agents == 0 {
            return Err("Swarm must allow at least one agent".to_string());
        }
        
        if swarm.agents.len() > swarm.max_agents as usize {
            return Err(format!(
                "Swarm has {} agents but max is {}",
                swarm.agents.len(),
                swarm.max_agents
            ));
        }
        
        Ok(())
    }
}

/// Async test utilities
pub struct AsyncTestHelper;

impl AsyncTestHelper {
    /// Wait for condition with timeout
    pub async fn wait_for_condition<F>(
        mut condition: F,
        timeout_duration: Duration,
        check_interval: Duration,
    ) -> Result<(), String>
    where
        F: FnMut() -> bool,
    {
        let start = SystemTime::now();
        
        while start.elapsed().unwrap_or_default() < timeout_duration {
            if condition() {
                return Ok(());
            }
            tokio::time::sleep(check_interval).await;
        }
        
        Err(format!("Condition not met within {:?}", timeout_duration))
    }
    
    /// Execute operations concurrently and collect results
    pub async fn execute_concurrent<F, T>(
        operations: Vec<F>,
        max_concurrent: usize,
    ) -> Vec<T>
    where
        F: std::future::Future<Output = T>,
    {
        use futures::stream::{FuturesUnordered, StreamExt};
        
        let mut results = Vec::new();
        let mut futures = FuturesUnordered::new();
        let mut operations_iter = operations.into_iter();
        
        // Start initial batch
        for _ in 0..max_concurrent {
            if let Some(op) = operations_iter.next() {
                futures.push(op);
            }
        }
        
        // Process results and start new operations
        while let Some(result) = futures.next().await {
            results.push(result);
            
            if let Some(op) = operations_iter.next() {
                futures.push(op);
            }
        }
        
        results
    }
}

/// Test fixture utilities
pub struct TestFixture<T> {
    setup_fn: Box<dyn Fn() -> T>,
    cleanup_fn: Option<Box<dyn Fn(&mut T)>>,
}

impl<T> TestFixture<T> {
    pub fn new<F>(setup: F) -> Self
    where
        F: Fn() -> T + 'static,
    {
        Self {
            setup_fn: Box::new(setup),
            cleanup_fn: None,
        }
    }
    
    pub fn with_cleanup<F>(mut self, cleanup: F) -> Self
    where
        F: Fn(&mut T) + 'static,
    {
        self.cleanup_fn = Some(Box::new(cleanup));
        self
    }
    
    pub fn create(&self) -> TestFixtureGuard<T> {
        let instance = (self.setup_fn)();
        TestFixtureGuard {
            instance,
            cleanup_fn: self.cleanup_fn.as_ref(),
        }
    }
}

pub struct TestFixtureGuard<'a, T> {
    instance: T,
    cleanup_fn: Option<&'a Box<dyn Fn(&mut T)>>,
}

impl<T> std::ops::Deref for TestFixtureGuard<'_, T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        &self.instance
    }
}

impl<T> std::ops::DerefMut for TestFixtureGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.instance
    }
}

impl<T> Drop for TestFixtureGuard<'_, T> {
    fn drop(&mut self) {
        if let Some(cleanup) = &self.cleanup_fn {
            cleanup(&mut self.instance);
        }
    }
}

#[cfg(test)]
mod test_utils_tests {
    use super::*;
    
    #[test]
    fn test_mock_verifier_interactions() {
        let calls = vec![
            SwarmCall {
                method: "init_swarm".to_string(),
                parameters: HashMap::new(),
                timestamp: SystemTime::now(),
                result: Ok("success".to_string()),
            },
            SwarmCall {
                method: "add_agent".to_string(),
                parameters: HashMap::new(),
                timestamp: SystemTime::now(),
                result: Ok("success".to_string()),
            },
        ];
        
        let result = MockVerifier::verify_interactions(&calls, &["init", "add"]);
        assert!(result.is_ok());
        
        let result = MockVerifier::verify_interactions(&calls, &["missing"]);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_performance_measurement() {
        let mut measurement = PerformanceMeasurement::start();
        
        measurement.measure("test_op", || {
            std::thread::sleep(Duration::from_millis(10));
        });
        
        let duration = measurement.get_measurement("test_op").unwrap();
        assert!(duration >= Duration::from_millis(10));
        
        let result = measurement.assert_performance_threshold("test_op", Duration::from_millis(100));
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_data_validator() {
        let valid_agent = MockAgent {
            id: "test-agent".to_string(),
            agent_type: AgentType::TddLondon,
            status: AgentStatus::Active,
            capabilities: vec!["testing".to_string()],
            memory_usage: 256,
            cpu_usage: 0.5,
            last_heartbeat: SystemTime::now(),
        };
        
        assert!(TestDataValidator::validate_agent(&valid_agent).is_ok());
        
        let invalid_agent = MockAgent {
            id: "".to_string(),
            agent_type: AgentType::TddLondon,
            status: AgentStatus::Active,
            capabilities: vec![],
            memory_usage: 0,
            cpu_usage: 2.0,
            last_heartbeat: SystemTime::now(),
        };
        
        assert!(TestDataValidator::validate_agent(&invalid_agent).is_err());
    }
}