//! Configuration validation utilities

use crate::{ConfigError, Result};
use std::collections::HashMap;

/// Trait for validating configuration values
pub trait ConfigValidator<T> {
    /// Validate a configuration value
    fn validate(&self, value: &T) -> Result<()>;
}

/// Validation rule for configuration keys
#[derive(Debug, Clone)]
pub enum ValidationRule {
    /// Value must be present
    Required,
    /// Value must be within a range (for numbers)
    Range { min: f64, max: f64 },
    /// Value must be one of the specified options
    OneOf(Vec<String>),
    /// Value must match a pattern (simplified)
    Pattern(String),
    /// Value must be a valid URL
    Url,
    /// Value must be a valid email
    Email,
    /// Custom validation function
    Custom(fn(&serde_json::Value) -> Result<()>),
}

/// Configuration validator that applies rules to configuration keys
pub struct ConfigRuleValidator {
    rules: HashMap<String, Vec<ValidationRule>>,
}

impl ConfigRuleValidator {
    /// Create a new validator
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
        }
    }

    /// Add a validation rule for a key
    pub fn add_rule(mut self, key: &str, rule: ValidationRule) -> Self {
        self.rules.entry(key.to_string()).or_default().push(rule);
        self
    }

    /// Add multiple rules for a key
    pub fn add_rules(mut self, key: &str, rules: Vec<ValidationRule>) -> Self {
        self.rules.entry(key.to_string()).or_default().extend(rules);
        self
    }

    /// Validate a configuration map
    pub fn validate_config(&self, config: &HashMap<String, serde_json::Value>) -> Result<()> {
        let mut errors = Vec::new();

        for (key, rules) in &self.rules {
            for rule in rules {
                if let Err(e) = self.validate_key_rule(config, key, rule) {
                    errors.push(format!("{}: {}", key, e));
                }
            }
        }

        if !errors.is_empty() {
            return Err(ConfigError::validation(errors.join(", ")));
        }

        Ok(())
    }

    fn validate_key_rule(
        &self,
        config: &HashMap<String, serde_json::Value>,
        key: &str,
        rule: &ValidationRule,
    ) -> Result<()> {
        match rule {
            ValidationRule::Required => {
                if !config.contains_key(key) {
                    return Err(ConfigError::validation(format!("Missing required key: {}", key)));
                }
            }
            ValidationRule::Range { min, max } => {
                if let Some(value) = config.get(key) {
                    if let Some(num) = value.as_f64() {
                        if num < *min || num > *max {
                            return Err(ConfigError::validation(format!(
                                "Value {} is not in range [{}, {}]",
                                num, min, max
                            )));
                        }
                    } else {
                        return Err(ConfigError::validation(format!(
                            "Value for {} is not a number",
                            key
                        )));
                    }
                }
            }
            ValidationRule::OneOf(options) => {
                if let Some(value) = config.get(key) {
                    if let Some(str_val) = value.as_str() {
                        if !options.contains(&str_val.to_string()) {
                            return Err(ConfigError::validation(format!(
                                "Value '{}' is not one of: {:?}",
                                str_val, options
                            )));
                        }
                    } else {
                        return Err(ConfigError::validation(format!(
                            "Value for {} is not a string",
                            key
                        )));
                    }
                }
            }
            ValidationRule::Pattern(pattern) => {
                if let Some(value) = config.get(key) {
                    if let Some(str_val) = value.as_str() {
                        // Simple pattern matching without regex for now
                        // Can be enhanced with regex crate if needed
                        if pattern == "*" || str_val.contains(pattern) {
                            // Basic pattern matching
                        } else {
                            return Err(ConfigError::validation(format!(
                                "Value '{}' does not match pattern '{}'",
                                str_val, pattern
                            )));
                        }
                    } else {
                        return Err(ConfigError::validation(format!(
                            "Value for {} is not a string",
                            key
                        )));
                    }
                }
            }
            ValidationRule::Url => {
                if let Some(value) = config.get(key) {
                    if let Some(str_val) = value.as_str() {
                        // Basic URL validation
                        if !str_val.starts_with("http://") && !str_val.starts_with("https://") {
                            return Err(ConfigError::validation(format!(
                                "Invalid URL format: {}. Must start with http:// or https://",
                                str_val
                            )));
                        }
                    } else {
                        return Err(ConfigError::validation(format!(
                            "Value for {} is not a string",
                            key
                        )));
                    }
                }
            }
            ValidationRule::Email => {
                if let Some(value) = config.get(key) {
                    if let Some(str_val) = value.as_str() {
                        if !str_val.contains('@') || !str_val.contains('.') {
                            return Err(ConfigError::validation(format!(
                                "Invalid email format: {}",
                                str_val
                            )));
                        }
                    } else {
                        return Err(ConfigError::validation(format!(
                            "Value for {} is not a string",
                            key
                        )));
                    }
                }
            }
            ValidationRule::Custom(validator_fn) => {
                if let Some(value) = config.get(key) {
                    validator_fn(value)?;
                }
            }
        }

        Ok(())
    }
}

impl Default for ConfigRuleValidator {
    fn default() -> Self {
        Self::new()
    }
}