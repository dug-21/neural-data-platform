use crate::ConfigError;

#[derive(Debug)]
pub struct ErrorSanitizer {
    is_production: bool,
}

impl ErrorSanitizer {
    pub fn new(is_production: bool) -> Self {
        Self { is_production }
    }

    pub fn sanitize(&self, error: ConfigError) -> ConfigError {
        if !self.is_production {
            // In development, return the original error
            return error;
        }

        // In production, sanitize the error
        match error {
            ConfigError::Io(_) => {
                // Don't expose file paths or system details
                ConfigError::Io("Configuration not found or inaccessible".to_string())
            },
            ConfigError::Parse(msg) => {
                // Don't expose parsing details that might reveal structure
                if msg.contains("line") || msg.contains("column") || msg.contains("/") {
                    ConfigError::Parse("Invalid configuration format".to_string())
                } else {
                    ConfigError::Parse("Invalid configuration format".to_string())
                }
            },
            ConfigError::ValidationFailed(ref messages) => {
                // Sanitize validation errors to avoid exposing internals
                let sanitized_messages: Vec<String> = messages.iter()
                    .map(|msg| {
                        if msg.contains("path") || msg.contains("/") || msg.contains("..") {
                            "Invalid configuration value".to_string()
                        } else if msg.to_lowercase().contains("secret") || msg.to_lowercase().contains("password") {
                            // Keep the secret blocking message as it's intentional
                            msg.clone()
                        } else {
                            "Invalid configuration value".to_string()
                        }
                    })
                    .collect();
                
                ConfigError::ValidationFailed(sanitized_messages)
            },
            ConfigError::NotFound(_) => ConfigError::NotFound("Configuration not found".to_string()),
            ConfigError::TypeMismatch { expected, actual } => {
                // Keep type mismatch info as it's useful and not sensitive
                ConfigError::TypeMismatch { expected, actual }
            },
            _ => {
                // For any other error types, return a generic message
                ConfigError::ValidationFailed(vec!["Configuration error".to_string()])
            }
        }
    }
}

impl Default for ErrorSanitizer {
    fn default() -> Self {
        // Default to production mode for safety
        Self::new(true)
    }
}