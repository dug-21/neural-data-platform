# Security Fixes Implementation Guide

## Priority 1: Critical Fixes (Week 1)

### 1. Add Authentication & Authorization

```rust
// src/auth/mod.rs
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub role: Role,
    pub exp: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Role {
    Admin,
    ReadWrite,
    ReadOnly,
}

pub struct AuthMiddleware {
    secret: String,
}

impl AuthMiddleware {
    pub fn verify_token(&self, token: &str) -> Result<Claims, Error> {
        decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &Validation::default(),
        )
        .map(|data| data.claims)
        .map_err(|e| Error::Unauthorized(e.to_string()))
    }
    
    pub fn authorize(&self, claims: &Claims, operation: Operation) -> Result<(), Error> {
        match (claims.role, operation) {
            (Role::Admin, _) => Ok(()),
            (Role::ReadWrite, Operation::Read | Operation::Write) => Ok(()),
            (Role::ReadOnly, Operation::Read) => Ok(()),
            _ => Err(Error::Forbidden),
        }
    }
}
```

### 2. Implement Encryption for Secrets

```rust
// src/crypto/mod.rs
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce, Key
};
use base64::{Engine as _, engine::general_purpose};

pub struct SecretManager {
    cipher: Aes256Gcm,
}

impl SecretManager {
    pub fn new(key: &[u8; 32]) -> Self {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
        Self { cipher }
    }
    
    pub fn encrypt(&self, plaintext: &str) -> Result<String, Error> {
        let nonce = Nonce::from_slice(b"unique nonce"); // Use random nonce in production
        let ciphertext = self.cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| Error::Encryption(e.to_string()))?;
        
        Ok(general_purpose::STANDARD.encode(ciphertext))
    }
    
    pub fn decrypt(&self, ciphertext: &str) -> Result<String, Error> {
        let decoded = general_purpose::STANDARD
            .decode(ciphertext)
            .map_err(|e| Error::Decryption(e.to_string()))?;
        
        let nonce = Nonce::from_slice(b"unique nonce");
        let plaintext = self.cipher
            .decrypt(nonce, decoded.as_ref())
            .map_err(|e| Error::Decryption(e.to_string()))?;
        
        String::from_utf8(plaintext)
            .map_err(|e| Error::Decryption(e.to_string()))
    }
}

// Integrate with ConfigStore
impl ConfigStore {
    pub fn set_secret(&mut self, key: &str, value: &str) -> Result<(), Error> {
        let encrypted = self.secret_manager.encrypt(value)?;
        self.data.insert(key.to_string(), ConfigValue::Secret(encrypted));
        Ok(())
    }
    
    pub fn get_secret(&self, key: &str) -> Result<String, Error> {
        match self.data.get(key) {
            Some(ConfigValue::Secret(encrypted)) => {
                self.secret_manager.decrypt(encrypted)
            }
            _ => Err(Error::NotFound),
        }
    }
}
```

### 3. Fix Path Traversal

```rust
// src/loader.rs - SECURE VERSION
use std::path::{Path, PathBuf};
use std::fs;

pub struct SecureLoader {
    allowed_dirs: Vec<PathBuf>,
}

impl SecureLoader {
    pub fn new(allowed_dirs: Vec<PathBuf>) -> Self {
        Self { allowed_dirs }
    }
    
    pub fn load_from_file(&self, file_path: &str) -> Result<String, Error> {
        let path = Path::new(file_path);
        
        // Canonicalize to resolve symlinks and relative paths
        let canonical_path = path.canonicalize()
            .map_err(|e| Error::InvalidPath(e.to_string()))?;
        
        // Check if path is within allowed directories
        let is_allowed = self.allowed_dirs.iter().any(|allowed| {
            canonical_path.starts_with(allowed)
        });
        
        if !is_allowed {
            return Err(Error::Unauthorized(
                "Access to path denied".to_string()
            ));
        }
        
        // Additional checks
        if canonical_path.to_string_lossy().contains("..") {
            return Err(Error::InvalidPath(
                "Path traversal detected".to_string()
            ));
        }
        
        fs::read_to_string(canonical_path)
            .map_err(|e| Error::Io(e))
    }
}
```

### 4. Safe JSON Deserialization

```rust
// src/loader.rs - SAFE DESERIALIZATION
use serde::de::DeserializeOwned;
use serde_json::Value;

const MAX_JSON_SIZE: usize = 10_485_760; // 10MB limit
const MAX_JSON_DEPTH: usize = 128;

pub fn safe_deserialize<T: DeserializeOwned>(json_str: &str) -> Result<T, Error> {
    // Size check
    if json_str.len() > MAX_JSON_SIZE {
        return Err(Error::Validation("JSON too large".to_string()));
    }
    
    // Parse to Value first for validation
    let value: Value = serde_json::from_str(json_str)
        .map_err(|e| Error::Deserialization(e.to_string()))?;
    
    // Check depth
    if json_depth(&value) > MAX_JSON_DEPTH {
        return Err(Error::Validation("JSON too deeply nested".to_string()));
    }
    
    // Validate against schema if available
    validate_schema(&value)?;
    
    // Deserialize with limits
    serde_json::from_value(value)
        .map_err(|e| Error::Deserialization(e.to_string()))
}

fn json_depth(value: &Value) -> usize {
    match value {
        Value::Object(map) => {
            1 + map.values().map(json_depth).max().unwrap_or(0)
        }
        Value::Array(arr) => {
            1 + arr.iter().map(json_depth).max().unwrap_or(0)
        }
        _ => 1,
    }
}
```

## Priority 2: High Severity Fixes (Week 2)

### 5. Add Input Validation

```rust
// src/validator.rs - COMPREHENSIVE VALIDATION
use regex::Regex;
use once_cell::sync::Lazy;

static KEY_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-zA-Z0-9_.-]+(/[a-zA-Z0-9_.-]+)*$").unwrap()
});

static SQL_INJECTION_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(select|insert|update|delete|drop|union|exec|script)").unwrap()
});

pub struct Validator {
    max_key_length: usize,
    max_value_size: usize,
}

impl Validator {
    pub fn validate_key(&self, key: &str) -> Result<(), Error> {
        // Length check
        if key.is_empty() || key.len() > self.max_key_length {
            return Err(Error::Validation("Invalid key length".to_string()));
        }
        
        // Format check
        if !KEY_REGEX.is_match(key) {
            return Err(Error::Validation("Invalid key format".to_string()));
        }
        
        // SQL injection check
        if SQL_INJECTION_PATTERN.is_match(key) {
            return Err(Error::Validation("Potential injection detected".to_string()));
        }
        
        // Path traversal check
        if key.contains("..") || key.contains("~") {
            return Err(Error::Validation("Invalid characters in key".to_string()));
        }
        
        Ok(())
    }
    
    pub fn validate_value(&self, value: &ConfigValue) -> Result<(), Error> {
        match value {
            ConfigValue::String(s) => {
                if s.len() > self.max_value_size {
                    return Err(Error::Validation("Value too large".to_string()));
                }
                // Check for script injection
                if s.contains("<script") || s.contains("javascript:") {
                    return Err(Error::Validation("Script injection detected".to_string()));
                }
            }
            ConfigValue::Object(map) => {
                // Recursively validate nested objects
                for (k, v) in map {
                    self.validate_key(k)?;
                    self.validate_value(v)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
```

### 6. Fix Race Conditions

```rust
// src/async_store.rs - THREAD-SAFE VERSION
use tokio::sync::{RwLock, Mutex};
use std::sync::Arc;

pub struct ThreadSafeAsyncStore {
    data: Arc<RwLock<HashMap<String, ConfigValue>>>,
    write_lock: Arc<Mutex<()>>,
}

impl ThreadSafeAsyncStore {
    pub async fn get(&self, key: &str) -> Option<ConfigValue> {
        let data = self.data.read().await;
        data.get(key).cloned()
    }
    
    pub async fn set(&self, key: String, value: ConfigValue) -> Result<(), Error> {
        // Acquire write lock for atomic operations
        let _guard = self.write_lock.lock().await;
        
        // Validate before writing
        self.validator.validate_key(&key)?;
        self.validator.validate_value(&value)?;
        
        let mut data = self.data.write().await;
        data.insert(key, value);
        
        // Audit log
        self.audit_log.log_write(&key).await?;
        
        Ok(())
    }
    
    pub async fn transaction<F, R>(&self, f: F) -> Result<R, Error>
    where
        F: FnOnce(&mut HashMap<String, ConfigValue>) -> Result<R, Error>,
    {
        let _guard = self.write_lock.lock().await;
        let mut data = self.data.write().await;
        f(&mut *data)
    }
}
```

### 7. Implement Rate Limiting

```rust
// src/middleware/rate_limit.rs
use std::time::{Duration, Instant};
use dashmap::DashMap;

pub struct RateLimiter {
    limits: DashMap<String, WindowCounter>,
    max_requests: u32,
    window: Duration,
}

struct WindowCounter {
    count: u32,
    window_start: Instant,
}

impl RateLimiter {
    pub fn check_rate_limit(&self, client_id: &str) -> Result<(), Error> {
        let mut entry = self.limits.entry(client_id.to_string())
            .or_insert_with(|| WindowCounter {
                count: 0,
                window_start: Instant::now(),
            });
        
        let now = Instant::now();
        
        // Reset window if expired
        if now.duration_since(entry.window_start) > self.window {
            entry.count = 0;
            entry.window_start = now;
        }
        
        // Check limit
        if entry.count >= self.max_requests {
            return Err(Error::RateLimitExceeded);
        }
        
        entry.count += 1;
        Ok(())
    }
}
```

## Priority 3: Medium Severity Fixes (Week 3)

### 8. Add Audit Logging

```rust
// src/audit/mod.rs
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Serialize)]
pub struct AuditEntry {
    timestamp: DateTime<Utc>,
    user_id: String,
    action: Action,
    resource: String,
    result: Result<String, String>,
    ip_address: Option<String>,
    user_agent: Option<String>,
}

pub struct AuditLogger {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl AuditLogger {
    pub async fn log(&self, entry: AuditEntry) -> Result<(), Error> {
        let json = serde_json::to_string(&entry)?;
        let mut writer = self.writer.lock().await;
        writeln!(writer, "{}", json)?;
        writer.flush()?;
        Ok(())
    }
    
    pub async fn log_access(&self, user: &str, resource: &str, granted: bool) {
        let entry = AuditEntry {
            timestamp: Utc::now(),
            user_id: user.to_string(),
            action: Action::Access,
            resource: resource.to_string(),
            result: if granted { Ok("granted".to_string()) } else { Err("denied".to_string()) },
            ip_address: None,
            user_agent: None,
        };
        let _ = self.log(entry).await;
    }
}
```

### 9. Secure Configuration Defaults

```rust
// src/config/defaults.rs
pub struct SecureDefaults;

impl SecureDefaults {
    pub fn apply() -> ConfigStore {
        let mut store = ConfigStore::new();
        
        // Security settings
        store.set("security.require_authentication", ConfigValue::Bool(true));
        store.set("security.encryption_enabled", ConfigValue::Bool(true));
        store.set("security.audit_logging", ConfigValue::Bool(true));
        store.set("security.rate_limit_enabled", ConfigValue::Bool(true));
        store.set("security.max_requests_per_minute", ConfigValue::Integer(60));
        
        // Session settings
        store.set("session.timeout_minutes", ConfigValue::Integer(30));
        store.set("session.secure_cookies", ConfigValue::Bool(true));
        store.set("session.same_site", ConfigValue::String("strict".to_string()));
        
        // Validation settings
        store.set("validation.max_key_length", ConfigValue::Integer(256));
        store.set("validation.max_value_size", ConfigValue::Integer(1048576)); // 1MB
        
        store
    }
}
```

## Testing Security Fixes

```rust
#[cfg(test)]
mod security_tests {
    use super::*;
    
    #[test]
    fn test_path_traversal_blocked() {
        let loader = SecureLoader::new(vec![PathBuf::from("/allowed")]);
        assert!(loader.load_from_file("../../etc/passwd").is_err());
        assert!(loader.load_from_file("/allowed/config.json").is_ok());
    }
    
    #[test]
    fn test_sql_injection_blocked() {
        let validator = Validator::default();
        assert!(validator.validate_key("'; DROP TABLE users; --").is_err());
    }
    
    #[test]
    fn test_encryption_decryption() {
        let manager = SecretManager::new(&[0u8; 32]);
        let secret = "my-secret-api-key";
        let encrypted = manager.encrypt(secret).unwrap();
        assert_ne!(encrypted, secret);
        let decrypted = manager.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, secret);
    }
    
    #[tokio::test]
    async fn test_rate_limiting() {
        let limiter = RateLimiter::new(3, Duration::from_secs(60));
        
        for _ in 0..3 {
            assert!(limiter.check_rate_limit("client1").is_ok());
        }
        
        // Fourth request should be blocked
        assert!(limiter.check_rate_limit("client1").is_err());
    }
}
```

## Deployment Checklist

- [ ] All critical vulnerabilities fixed
- [ ] Security tests passing
- [ ] External security audit completed
- [ ] Penetration testing performed
- [ ] Security documentation updated
- [ ] Incident response plan in place
- [ ] Monitoring and alerting configured
- [ ] Backup and recovery tested
- [ ] Compliance requirements met
- [ ] Security training completed

---

*Implementation guide for config-store security fixes*  
*Estimated completion: 4 weeks with dedicated security team*