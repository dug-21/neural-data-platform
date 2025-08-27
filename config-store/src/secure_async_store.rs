use crate::{ConfigValue, ConfigError};
use crate::security::{SecretBlocker, InputValidator, RateLimiter, ErrorSanitizer};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use std::time::Duration;

/// Thread-safe async configuration store with security features
pub struct AsyncConfigStore {
    data: Arc<RwLock<HashMap<String, ConfigValue>>>,
    write_lock: Arc<Mutex<()>>,
    secret_blocker: SecretBlocker,
    validator: InputValidator,
    rate_limiter: Option<RateLimiter>,
    error_sanitizer: ErrorSanitizer,
}

impl AsyncConfigStore {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            write_lock: Arc::new(Mutex::new(())),
            secret_blocker: SecretBlocker::new(),
            validator: InputValidator::new(),
            rate_limiter: None,
            error_sanitizer: ErrorSanitizer::new(false), // Development mode by default
        }
    }

    pub fn with_rate_limiting(mut self, max_requests: u32, window: Duration) -> Self {
        self.rate_limiter = Some(RateLimiter::new(max_requests, window));
        self
    }

    pub fn with_production_mode(mut self) -> Self {
        self.error_sanitizer = ErrorSanitizer::new(true);
        self
    }

    pub async fn get(&self, key: &str) -> Option<ConfigValue> {
        // Validate key
        if let Err(_) = self.validator.validate_key(key) {
            return None;
        }

        let data = self.data.read().await;
        data.get(key).cloned()
    }

    pub async fn set(&self, key: &str, value: ConfigValue) -> Result<(), ConfigError> {
        // Check for secrets
        self.secret_blocker.check_value(key, &value)
            .map_err(|e| self.error_sanitizer.sanitize(e))?;

        // Validate input
        self.validator.validate_key(key)
            .map_err(|e| self.error_sanitizer.sanitize(e))?;
        self.validator.validate_value(&value)
            .map_err(|e| self.error_sanitizer.sanitize(e))?;

        // Write lock for exclusive write
        let _guard = self.write_lock.lock().await;

        // Get write access to data
        let mut data = self.data.write().await;
        data.insert(key.to_string(), value);

        Ok(())
    }

    pub async fn set_with_client(&self, client_id: &str, key: &str, value: ConfigValue) -> Result<(), ConfigError> {
        // Check rate limit if enabled
        if let Some(ref limiter) = self.rate_limiter {
            limiter.check(client_id)
                .map_err(|e| self.error_sanitizer.sanitize(e))?;
        }

        self.set(key, value).await
    }

    pub async fn get_with_client(&self, client_id: &str, key: &str) -> Result<Option<ConfigValue>, ConfigError> {
        // Check rate limit if enabled
        if let Some(ref limiter) = self.rate_limiter {
            limiter.check(client_id)
                .map_err(|e| self.error_sanitizer.sanitize(e))?;
        }

        Ok(self.get(key).await)
    }

    pub async fn delete(&self, key: &str) -> Result<(), ConfigError> {
        // Validate key
        self.validator.validate_key(key)
            .map_err(|e| self.error_sanitizer.sanitize(e))?;

        let _guard = self.write_lock.lock().await;
        let mut data = self.data.write().await;
        
        data.remove(key)
            .ok_or_else(|| ConfigError::NotFound(key.to_string()))
            .map(|_| ())
            .map_err(|e| self.error_sanitizer.sanitize(e))
    }

    pub async fn update_atomic<F>(&self, key: &str, updater: F) -> Result<(), ConfigError>
    where
        F: FnOnce(Option<&ConfigValue>) -> Result<ConfigValue, ConfigError>,
    {
        // Exclusive lock for atomic update
        let _guard = self.write_lock.lock().await;
        let mut data = self.data.write().await;

        let current = data.get(key);
        let new_value = updater(current)?;

        // Validate new value
        self.secret_blocker.check_value(key, &new_value)
            .map_err(|e| self.error_sanitizer.sanitize(e))?;
        self.validator.validate_value(&new_value)
            .map_err(|e| self.error_sanitizer.sanitize(e))?;

        data.insert(key.to_string(), new_value);
        Ok(())
    }

    pub async fn len(&self) -> usize {
        let data = self.data.read().await;
        data.len()
    }

    pub async fn clear(&self) {
        let _guard = self.write_lock.lock().await;
        let mut data = self.data.write().await;
        data.clear();
    }
}

impl Default for AsyncConfigStore {
    fn default() -> Self {
        Self::new()
    }
}