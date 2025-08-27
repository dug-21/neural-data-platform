use crate::{ConfigValue, ConfigError};
use serde_json::Value;

pub struct SafeJsonParser {
    max_size: usize,
    max_depth: usize,
    max_keys: usize,
}

impl SafeJsonParser {
    pub fn new() -> Self {
        Self {
            max_size: 10_485_760,  // 10MB
            max_depth: 128,
            max_keys: 10_000,
        }
    }

    pub fn parse(&self, json_str: &str) -> Result<ConfigValue, ConfigError> {
        // Size check
        if json_str.len() > self.max_size {
            return Err(ConfigError::Parse(
                format!("JSON exceeds maximum size of {} bytes", self.max_size)
            ));
        }

        // Depth check
        let depth = self.calculate_depth(json_str)?;
        if depth > self.max_depth {
            return Err(ConfigError::Parse(
                format!("JSON nesting exceeds maximum depth of {}", self.max_depth)
            ));
        }

        // Parse to Value first for validation
        let value: Value = serde_json::from_str(json_str)
            .map_err(|e| ConfigError::Parse(format!("Invalid JSON: {}", e)))?;

        // Check key count
        let key_count = self.count_keys(&value);
        if key_count > self.max_keys {
            return Err(ConfigError::Parse(
                format!("JSON has too many keys ({}), maximum is {}", key_count, self.max_keys)
            ));
        }

        // Convert to ConfigValue
        self.value_to_config(value)
    }

    fn calculate_depth(&self, json: &str) -> Result<usize, ConfigError> {
        let mut max_depth = 0;
        let mut current_depth = 0;
        let mut in_string = false;
        let mut escape_next = false;

        for ch in json.chars() {
            if escape_next {
                escape_next = false;
                continue;
            }

            if ch == '\\' && in_string {
                escape_next = true;
                continue;
            }

            if ch == '"' && !in_string {
                in_string = true;
            } else if ch == '"' && in_string {
                in_string = false;
            } else if !in_string {
                match ch {
                    '{' | '[' => {
                        current_depth += 1;
                        max_depth = max_depth.max(current_depth);
                    },
                    '}' | ']' => {
                        if current_depth == 0 {
                            return Err(ConfigError::Parse("Unbalanced brackets in JSON".to_string()));
                        }
                        current_depth -= 1;
                    },
                    _ => {}
                }
            }
        }

        if current_depth != 0 {
            return Err(ConfigError::Parse("Unbalanced brackets in JSON".to_string()));
        }

        Ok(max_depth)
    }

    fn count_keys(&self, value: &Value) -> usize {
        match value {
            Value::Object(map) => {
                let mut count = map.len();
                for v in map.values() {
                    count += self.count_keys(v);
                }
                count
            },
            Value::Array(arr) => {
                arr.iter().map(|v| self.count_keys(v)).sum()
            },
            _ => 0
        }
    }

    fn value_to_config(&self, value: Value) -> Result<ConfigValue, ConfigError> {
        match value {
            Value::Null => Ok(ConfigValue::Null),
            Value::Bool(b) => Ok(ConfigValue::Boolean(b)),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(ConfigValue::Integer(i))
                } else if let Some(f) = n.as_f64() {
                    if f.is_finite() {
                        Ok(ConfigValue::Float(f))
                    } else {
                        Err(ConfigError::Parse("Invalid number: infinite or NaN".to_string()))
                    }
                } else {
                    Err(ConfigError::Parse("Invalid number format".to_string()))
                }
            },
            Value::String(s) => Ok(ConfigValue::String(s)),
            Value::Array(arr) => {
                let config_arr = arr.into_iter()
                    .map(|v| self.value_to_config(v))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(ConfigValue::Array(config_arr))
            },
            Value::Object(map) => {
                let mut config_map = HashMap::new();
                for (k, v) in map {
                    config_map.insert(k, self.value_to_config(v)?);
                }
                Ok(ConfigValue::Object(config_map))
            }
        }
    }
}

impl Default for SafeJsonParser {
    fn default() -> Self {
        Self::new()
    }
}

use std::collections::HashMap;