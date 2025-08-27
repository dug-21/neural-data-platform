use crate::{ConfigValue, ConfigError};
use regex::Regex;
use once_cell::sync::Lazy;

static KEY_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^/[a-zA-Z0-9_.-]+(/[a-zA-Z0-9_.-]+)*$").unwrap()
});

static INJECTION_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r"(\.\./)+").unwrap(),              // Path traversal
        Regex::new(r"';|--|/\*|\*/").unwrap(),         // SQL injection
        Regex::new(r"<script|javascript:").unwrap(),   // XSS
        Regex::new(r"\$\{|\$\(").unwrap(),            // Command injection
        Regex::new(r"\{'\$ne").unwrap(),              // NoSQL injection
    ]
});

#[derive(Debug)]
pub struct InputValidator {
    max_key_length: usize,
    max_value_size: usize,
    max_object_keys: usize,
    max_array_size: usize,
}

impl InputValidator {
    pub fn new() -> Self {
        Self {
            max_key_length: 256,
            max_value_size: 1_048_576,  // 1MB
            max_object_keys: 1000,
            max_array_size: 10000,
        }
    }

    pub fn validate_key(&self, key: &str) -> Result<(), ConfigError> {
        // Length check
        if key.is_empty() || key.len() > self.max_key_length {
            return Err(ConfigError::ValidationFailed(
                format!("Key length must be between 1 and {} characters", self.max_key_length)
            ));
        }

        // Format check
        if !KEY_PATTERN.is_match(key) {
            return Err(ConfigError::ValidationFailed(
                "Key contains invalid characters. Only alphanumeric, _, -, ., and / are allowed".to_string()
            ));
        }

        // Injection check
        for pattern in INJECTION_PATTERNS.iter() {
            if pattern.is_match(key) {
                return Err(ConfigError::ValidationFailed(
                    "Potential injection pattern detected in key".to_string()
                ));
            }
        }

        Ok(())
    }

    pub fn validate_value(&self, value: &ConfigValue) -> Result<(), ConfigError> {
        match value {
            ConfigValue::String(s) => {
                // Size check
                if s.len() > self.max_value_size {
                    return Err(ConfigError::ValidationFailed(
                        format!("String value exceeds maximum size of {} bytes", self.max_value_size)
                    ));
                }

                // Injection patterns check
                for pattern in INJECTION_PATTERNS.iter() {
                    if pattern.is_match(s) {
                        return Err(ConfigError::ValidationFailed(
                            "Potential injection pattern detected in value".to_string()
                        ));
                    }
                }

                Ok(())
            },
            ConfigValue::Integer(_) => Ok(()),
            ConfigValue::Float(f) => {
                if !f.is_finite() {
                    return Err(ConfigError::ValidationFailed(
                        "Invalid number: infinite or NaN values not allowed".to_string()
                    ));
                }
                Ok(())
            },
            ConfigValue::Boolean(_) | ConfigValue::Null => Ok(()),
            ConfigValue::Object(map) => {
                // Check object size
                if map.len() > self.max_object_keys {
                    return Err(ConfigError::ValidationFailed(
                        format!("Object has too many keys ({}), maximum is {}", 
                                map.len(), self.max_object_keys)
                    ));
                }

                // Validate each key and value
                for (k, v) in map {
                    self.validate_key(k)?;
                    self.validate_value(v)?;
                }

                Ok(())
            },
            ConfigValue::Array(arr) => {
                // Check array size
                if arr.len() > self.max_array_size {
                    return Err(ConfigError::ValidationFailed(
                        format!("Array too large ({}), maximum is {} items", 
                                arr.len(), self.max_array_size)
                    ));
                }

                // Validate each item
                for item in arr {
                    self.validate_value(item)?;
                }

                Ok(())
            },
        }
    }
}

impl Default for InputValidator {
    fn default() -> Self {
        Self::new()
    }
}