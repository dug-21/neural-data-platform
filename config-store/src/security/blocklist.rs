use crate::{ConfigValue, ConfigError};
use regex::Regex;

#[derive(Debug)]
pub struct SecretBlocker {
    blocked_key_patterns: Vec<&'static str>,
    value_patterns: Vec<Regex>,
}

impl SecretBlocker {
    pub fn new() -> Self {
        Self {
            blocked_key_patterns: vec![
                "password", "passwd", "pwd",
                "secret", "api_key", "apikey",
                "token", "auth", "credential",
                "private_key", "privatekey", "privkey",
                "client_secret", "access_token", "refresh_token",
                "bearer_token", "auth_token",
            ],
            value_patterns: vec![
                // Stripe keys
                Regex::new(r"^sk_(live|test)_[a-zA-Z0-9]+").unwrap(),
                Regex::new(r"^pk_(live|test)_[a-zA-Z0-9]+").unwrap(),
                // GitHub tokens
                Regex::new(r"^ghp_[a-zA-Z0-9]{36}$").unwrap(),
                // AWS keys (simplified)
                Regex::new(r"^AKIA[A-Z0-9]{16}$").unwrap(),
                // Base64 secrets (40+ chars)
                Regex::new(r"^[A-Za-z0-9+/]{40,}={0,2}$").unwrap(),
            ],
        }
    }

    pub fn is_blocked_key(&self, key: &str) -> bool {
        let key_lower = key.to_lowercase();
        self.blocked_key_patterns.iter().any(|pattern| {
            key_lower.contains(pattern)
        })
    }

    pub fn is_blocked_value(&self, value: &str) -> bool {
        self.value_patterns.iter().any(|pattern| {
            pattern.is_match(value)
        })
    }

    pub fn check_value(&self, key: &str, value: &ConfigValue) -> Result<(), ConfigError> {
        // Check the key itself
        if self.is_blocked_key(key) {
            return Err(ConfigError::ValidationFailed(
                vec![format!("Secrets/passwords cannot be stored in config-store. Key '{}' appears to contain sensitive data.", key)]
            ));
        }

        // Check the value
        match value {
            ConfigValue::String(s) => {
                if self.is_blocked_value(s) {
                    return Err(ConfigError::ValidationFailed(
                        vec!["Value appears to be a secret/credential and cannot be stored".to_string()]
                    ));
                }
            },
            ConfigValue::Object(map) => {
                // Recursively check nested objects
                for (k, v) in map {
                    self.check_value(k, v)?;
                }
            },
            ConfigValue::Array(arr) => {
                // Check array elements
                for (idx, item) in arr.iter().enumerate() {
                    self.check_value(&format!("[{}]", idx), item)?;
                }
            },
            _ => {} // Numbers, booleans, null are fine
        }

        Ok(())
    }
}

impl Default for SecretBlocker {
    fn default() -> Self {
        Self::new()
    }
}