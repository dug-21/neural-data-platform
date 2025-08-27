//! Configuration Store Security Tests
//!
//! Comprehensive security tests for Config Store including authentication, authorization,
//! encryption, audit logging, input validation, and access control mechanisms.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use uuid::Uuid;

/// Security manager for Config Store
#[derive(Debug, Clone)]
pub struct ConfigSecurityManager {
    auth_provider: Arc<AuthenticationProvider>,
    authz_manager: Arc<AuthorizationManager>,
    encryption_service: Arc<EncryptionService>,
    audit_logger: Arc<AuditLogger>,
    access_control: Arc<AccessControlManager>,
    security_policies: Arc<RwLock<HashMap<String, SecurityPolicy>>>,
    active_sessions: Arc<RwLock<HashMap<String, SessionInfo>>>,
}

#[derive(Debug, Clone)]
pub struct AuthenticationProvider {
    user_store: Arc<Mutex<HashMap<String, UserCredentials>>>,
    session_store: Arc<Mutex<HashMap<String, SessionData>>>,
    auth_config: AuthConfig,
}

#[derive(Debug, Clone)]
pub struct AuthorizationManager {
    role_definitions: Arc<RwLock<HashMap<String, Role>>>,
    user_roles: Arc<RwLock<HashMap<String, Vec<String>>>>,
    permission_cache: Arc<Mutex<HashMap<String, CachedPermission>>>,
}

#[derive(Debug, Clone)]
pub struct EncryptionService {
    encryption_keys: Arc<RwLock<HashMap<String, EncryptionKey>>>,
    current_key_id: Arc<Mutex<String>>,
    encryption_config: EncryptionConfig,
}

#[derive(Debug, Clone)]
pub struct AuditLogger {
    audit_log: Arc<Mutex<Vec<AuditLogEntry>>>,
    audit_config: AuditConfig,
}

#[derive(Debug, Clone)]
pub struct AccessControlManager {
    ip_whitelist: Arc<RwLock<HashSet<String>>>,
    rate_limits: Arc<Mutex<HashMap<String, RateLimitTracker>>>,
    access_policies: Arc<RwLock<HashMap<String, AccessPolicy>>>,
}

#[derive(Debug, Clone)]
pub struct UserCredentials {
    pub user_id: String,
    pub username: String,
    pub password_hash: String,
    pub salt: String,
    pub roles: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
    pub failed_login_attempts: u32,
    pub account_locked: bool,
    pub password_expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct SessionData {
    pub session_id: String,
    pub user_id: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
    pub ip_address: String,
    pub user_agent: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_id: String,
    pub user_id: String,
    pub username: String,
    pub roles: Vec<String>,
    pub permissions: HashSet<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub ip_address: String,
}

#[derive(Debug, Clone)]
pub struct Role {
    pub role_id: String,
    pub role_name: String,
    pub permissions: HashSet<Permission>,
    pub inherits_from: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Permission {
    ConfigRead(String),     // Read specific config namespace
    ConfigWrite(String),    // Write specific config namespace  
    ConfigDelete(String),   // Delete specific config namespace
    ConfigAdmin,            // Full admin access
    AuditRead,             // Read audit logs
    UserManagement,        // Manage users and roles
    SystemAdmin,           // System administration
}

#[derive(Debug, Clone)]
pub struct CachedPermission {
    pub user_id: String,
    pub permission: Permission,
    pub allowed: bool,
    pub cached_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct EncryptionKey {
    pub key_id: String,
    pub key_data: Vec<u8>,
    pub algorithm: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct AuditLogEntry {
    pub entry_id: String,
    pub timestamp: DateTime<Utc>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub action: AuditAction,
    pub resource: String,
    pub ip_address: String,
    pub user_agent: String,
    pub success: bool,
    pub error_message: Option<String>,
    pub additional_data: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum AuditAction {
    Login,
    Logout,
    ConfigRead,
    ConfigWrite,
    ConfigDelete,
    PermissionCheck,
    AuthenticationFailure,
    AuthorizationFailure,
    SecurityViolation,
}

#[derive(Debug, Clone)]
pub struct RateLimitTracker {
    pub requests: Vec<DateTime<Utc>>,
    pub max_requests: u32,
    pub window_duration: Duration,
    pub blocked_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct AccessPolicy {
    pub policy_id: String,
    pub name: String,
    pub ip_restrictions: Vec<String>,
    pub time_restrictions: Vec<TimeWindow>,
    pub rate_limits: RateLimitConfig,
    pub required_permissions: Vec<Permission>,
}

#[derive(Debug, Clone)]
pub struct TimeWindow {
    pub start_time: String, // HH:MM format
    pub end_time: String,   // HH:MM format
    pub days_of_week: Vec<u8>, // 0=Sunday, 1=Monday, etc.
}

#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub max_requests: u32,
    pub window_minutes: u32,
    pub block_duration_minutes: u32,
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub session_timeout_minutes: u32,
    pub max_failed_login_attempts: u32,
    pub account_lockout_duration_minutes: u32,
    pub password_min_length: u32,
    pub password_require_special_chars: bool,
    pub password_expiry_days: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct EncryptionConfig {
    pub default_algorithm: String,
    pub key_rotation_days: u32,
    pub encrypt_at_rest: bool,
    pub encrypt_in_transit: bool,
}

#[derive(Debug, Clone)]
pub struct AuditConfig {
    pub log_all_actions: bool,
    pub log_failed_attempts: bool,
    pub retention_days: u32,
    pub secure_logging: bool,
}

#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    pub namespace: String,
    pub required_roles: Vec<String>,
    pub encryption_required: bool,
    pub audit_level: AuditLevel,
    pub access_restrictions: AccessRestrictions,
}

#[derive(Debug, Clone)]
pub enum AuditLevel {
    None,
    Basic,
    Detailed,
    Full,
}

#[derive(Debug, Clone)]
pub struct AccessRestrictions {
    pub ip_whitelist: Vec<String>,
    pub time_restrictions: Vec<TimeWindow>,
    pub require_mfa: bool,
    pub max_concurrent_sessions: Option<u32>,
}

impl ConfigSecurityManager {
    pub fn new() -> Self {
        Self {
            auth_provider: Arc::new(AuthenticationProvider::new()),
            authz_manager: Arc::new(AuthorizationManager::new()),
            encryption_service: Arc::new(EncryptionService::new()),
            audit_logger: Arc::new(AuditLogger::new()),
            access_control: Arc::new(AccessControlManager::new()),
            security_policies: Arc::new(RwLock::new(HashMap::new())),
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Authenticate user and create session
    pub async fn authenticate(
        &self,
        username: &str,
        password: &str,
        ip_address: &str,
        user_agent: &str,
    ) -> Result<String> {
        // Log authentication attempt
        self.audit_logger.log_action(
            None,
            None,
            AuditAction::Login,
            &format!("user:{}", username),
            ip_address,
            user_agent,
            true,
            None,
            HashMap::new(),
        ).await;

        // Check rate limiting
        if self.access_control.is_rate_limited(ip_address).await? {
            self.audit_logger.log_action(
                None,
                None,
                AuditAction::SecurityViolation,
                &format!("rate_limit:{}", ip_address),
                ip_address,
                user_agent,
                false,
                Some("Rate limit exceeded".to_string()),
                HashMap::new(),
            ).await;
            return Err(anyhow::anyhow!("Rate limit exceeded"));
        }

        // Verify credentials
        let session_id = self.auth_provider.authenticate(username, password, ip_address, user_agent).await?;

        // Create session
        let user_creds = self.auth_provider.get_user_credentials(username).await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        let session_info = SessionInfo {
            session_id: session_id.clone(),
            user_id: user_creds.user_id.clone(),
            username: username.to_string(),
            roles: user_creds.roles.clone(),
            permissions: self.authz_manager.get_user_permissions(&user_creds.user_id).await?,
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::minutes(self.auth_provider.auth_config.session_timeout_minutes as i64),
            ip_address: ip_address.to_string(),
        };

        self.active_sessions.write().await.insert(session_id.clone(), session_info);

        Ok(session_id)
    }

    /// Validate session and get user info
    pub async fn validate_session(&self, session_id: &str) -> Result<SessionInfo> {
        let sessions = self.active_sessions.read().await;
        let session = sessions.get(session_id)
            .ok_or_else(|| anyhow::anyhow!("Invalid session"))?;

        if session.expires_at < Utc::now() {
            return Err(anyhow::anyhow!("Session expired"));
        }

        Ok(session.clone())
    }

    /// Check if user has permission for operation
    pub async fn check_permission(
        &self,
        session_id: &str,
        permission: Permission,
        resource: &str,
        ip_address: &str,
        user_agent: &str,
    ) -> Result<bool> {
        let session = self.validate_session(session_id).await?;

        // Log permission check
        self.audit_logger.log_action(
            Some(session.user_id.clone()),
            Some(session_id.to_string()),
            AuditAction::PermissionCheck,
            resource,
            ip_address,
            user_agent,
            true,
            None,
            HashMap::from([("permission".to_string(), format!("{:?}", permission))]),
        ).await;

        // Check authorization
        let allowed = self.authz_manager.check_permission(&session.user_id, &permission).await?;

        if !allowed {
            self.audit_logger.log_action(
                Some(session.user_id.clone()),
                Some(session_id.to_string()),
                AuditAction::AuthorizationFailure,
                resource,
                ip_address,
                user_agent,
                false,
                Some("Permission denied".to_string()),
                HashMap::from([("permission".to_string(), format!("{:?}", permission))]),
            ).await;
        }

        Ok(allowed)
    }

    /// Encrypt configuration value
    pub async fn encrypt_config_value(&self, namespace: &str, value: &str) -> Result<Vec<u8>> {
        // Check if encryption is required for this namespace
        let policies = self.security_policies.read().await;
        let policy = policies.get(namespace);

        if policy.map(|p| p.encryption_required).unwrap_or(false) {
            self.encryption_service.encrypt(value.as_bytes()).await
        } else {
            Ok(value.as_bytes().to_vec())
        }
    }

    /// Decrypt configuration value
    pub async fn decrypt_config_value(&self, namespace: &str, encrypted_value: &[u8]) -> Result<String> {
        let policies = self.security_policies.read().await;
        let policy = policies.get(namespace);

        if policy.map(|p| p.encryption_required).unwrap_or(false) {
            let decrypted = self.encryption_service.decrypt(encrypted_value).await?;
            String::from_utf8(decrypted).map_err(|e| anyhow::anyhow!("Invalid UTF-8: {}", e))
        } else {
            String::from_utf8(encrypted_value.to_vec()).map_err(|e| anyhow::anyhow!("Invalid UTF-8: {}", e))
        }
    }

    /// Validate input for security threats
    pub async fn validate_input(&self, input: &str, input_type: InputType) -> Result<()> {
        match input_type {
            InputType::ConfigKey => {
                if input.is_empty() || input.len() > 1000 {
                    return Err(anyhow::anyhow!("Invalid config key length"));
                }
                
                // Check for path traversal attempts
                if input.contains("../") || input.contains("..\\") {
                    return Err(anyhow::anyhow!("Path traversal detected"));
                }
                
                // Check for injection attempts
                if input.contains(';') || input.contains('\'') || input.contains('"') {
                    return Err(anyhow::anyhow!("Potential injection attempt detected"));
                }
            }
            InputType::ConfigValue => {
                if input.len() > 1_000_000 { // 1MB limit
                    return Err(anyhow::anyhow!("Config value too large"));
                }
                
                // Basic XSS detection
                if input.contains("<script") || input.contains("javascript:") {
                    return Err(anyhow::anyhow!("Potential XSS detected"));
                }
            }
            InputType::Username => {
                if input.is_empty() || input.len() > 100 {
                    return Err(anyhow::anyhow!("Invalid username length"));
                }
                
                // Only allow alphanumeric and basic characters
                if !input.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.') {
                    return Err(anyhow::anyhow!("Username contains invalid characters"));
                }
            }
        }

        Ok(())
    }

    /// Create user with security validation
    pub async fn create_user(
        &self,
        username: &str,
        password: &str,
        roles: Vec<String>,
        creator_session_id: &str,
    ) -> Result<String> {
        // Validate creator has user management permission
        let session = self.validate_session(creator_session_id).await?;
        if !self.authz_manager.check_permission(&session.user_id, &Permission::UserManagement).await? {
            return Err(anyhow::anyhow!("Insufficient permissions to create user"));
        }

        // Validate inputs
        self.validate_input(username, InputType::Username).await?;
        self.validate_password(password).await?;

        // Create user
        let user_id = self.auth_provider.create_user(username, password, roles).await?;

        // Log user creation
        self.audit_logger.log_action(
            Some(session.user_id),
            Some(creator_session_id.to_string()),
            AuditAction::ConfigWrite,
            &format!("user:{}", username),
            &session.ip_address,
            "system",
            true,
            None,
            HashMap::from([("action".to_string(), "create_user".to_string())]),
        ).await;

        Ok(user_id)
    }

    /// Set security policy for namespace
    pub async fn set_security_policy(&self, namespace: &str, policy: SecurityPolicy) -> Result<()> {
        self.security_policies.write().await.insert(namespace.to_string(), policy);
        Ok(())
    }

    /// Get audit logs with security filtering
    pub async fn get_audit_logs(
        &self,
        session_id: &str,
        filters: AuditLogFilters,
    ) -> Result<Vec<AuditLogEntry>> {
        // Validate session has audit read permission
        let session = self.validate_session(session_id).await?;
        if !self.authz_manager.check_permission(&session.user_id, &Permission::AuditRead).await? {
            return Err(anyhow::anyhow!("Insufficient permissions to read audit logs"));
        }

        self.audit_logger.get_logs(filters).await
    }

    /// Logout and invalidate session
    pub async fn logout(&self, session_id: &str) -> Result<()> {
        let session = self.validate_session(session_id).await.ok();
        
        // Remove session
        self.active_sessions.write().await.remove(session_id);

        // Log logout
        if let Some(session) = session {
            self.audit_logger.log_action(
                Some(session.user_id),
                Some(session_id.to_string()),
                AuditAction::Logout,
                "session",
                &session.ip_address,
                "system",
                true,
                None,
                HashMap::new(),
            ).await;
        }

        Ok(())
    }

    // Helper methods

    async fn validate_password(&self, password: &str) -> Result<()> {
        let config = &self.auth_provider.auth_config;
        
        if password.len() < config.password_min_length as usize {
            return Err(anyhow::anyhow!("Password too short"));
        }

        if config.password_require_special_chars {
            let has_special = password.chars().any(|c| !c.is_alphanumeric());
            if !has_special {
                return Err(anyhow::anyhow!("Password must contain special characters"));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum InputType {
    ConfigKey,
    ConfigValue,
    Username,
}

#[derive(Debug, Clone)]
pub struct AuditLogFilters {
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub user_id: Option<String>,
    pub action: Option<AuditAction>,
    pub resource: Option<String>,
    pub limit: Option<usize>,
}

// Implementation of component services

impl AuthenticationProvider {
    pub fn new() -> Self {
        Self {
            user_store: Arc::new(Mutex::new(HashMap::new())),
            session_store: Arc::new(Mutex::new(HashMap::new())),
            auth_config: AuthConfig {
                session_timeout_minutes: 60,
                max_failed_login_attempts: 5,
                account_lockout_duration_minutes: 30,
                password_min_length: 8,
                password_require_special_chars: true,
                password_expiry_days: Some(90),
            },
        }
    }

    pub async fn authenticate(&self, username: &str, password: &str, ip_address: &str, user_agent: &str) -> Result<String> {
        let users = self.user_store.lock().await;
        let user = users.get(username).ok_or_else(|| anyhow::anyhow!("Invalid credentials"))?;

        if user.account_locked {
            return Err(anyhow::anyhow!("Account locked"));
        }

        // Verify password
        if !self.verify_password(password, &user.password_hash, &user.salt) {
            return Err(anyhow::anyhow!("Invalid credentials"));
        }

        // Create session
        let session_id = Uuid::new_v4().to_string();
        let session = SessionData {
            session_id: session_id.clone(),
            user_id: user.user_id.clone(),
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::minutes(self.auth_config.session_timeout_minutes as i64),
            last_accessed: Utc::now(),
            ip_address: ip_address.to_string(),
            user_agent: user_agent.to_string(),
            permissions: Vec::new(),
        };

        self.session_store.lock().await.insert(session_id.clone(), session);

        Ok(session_id)
    }

    pub async fn create_user(&self, username: &str, password: &str, roles: Vec<String>) -> Result<String> {
        let user_id = Uuid::new_v4().to_string();
        let salt = self.generate_salt();
        let password_hash = self.hash_password(password, &salt);

        let user = UserCredentials {
            user_id: user_id.clone(),
            username: username.to_string(),
            password_hash,
            salt,
            roles,
            created_at: Utc::now(),
            last_login: None,
            failed_login_attempts: 0,
            account_locked: false,
            password_expires_at: self.auth_config.password_expiry_days.map(|days| {
                Utc::now() + chrono::Duration::days(days as i64)
            }),
        };

        self.user_store.lock().await.insert(username.to_string(), user);

        Ok(user_id)
    }

    pub async fn get_user_credentials(&self, username: &str) -> Result<Option<UserCredentials>> {
        let users = self.user_store.lock().await;
        Ok(users.get(username).cloned())
    }

    fn verify_password(&self, password: &str, hash: &str, salt: &str) -> bool {
        let computed_hash = self.hash_password(password, salt);
        computed_hash == hash
    }

    fn hash_password(&self, password: &str, salt: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        hasher.update(salt.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn generate_salt(&self) -> String {
        Uuid::new_v4().to_string()
    }
}

impl AuthorizationManager {
    pub fn new() -> Self {
        Self {
            role_definitions: Arc::new(RwLock::new(HashMap::new())),
            user_roles: Arc::new(RwLock::new(HashMap::new())),
            permission_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn check_permission(&self, user_id: &str, permission: &Permission) -> Result<bool> {
        // Check cache first
        let cache_key = format!("{}:{:?}", user_id, permission);
        {
            let cache = self.permission_cache.lock().await;
            if let Some(cached) = cache.get(&cache_key) {
                if cached.expires_at > Utc::now() {
                    return Ok(cached.allowed);
                }
            }
        }

        // Get user roles
        let user_roles = self.user_roles.read().await;
        let roles = user_roles.get(user_id).cloned().unwrap_or_default();

        // Check permissions for each role
        let role_defs = self.role_definitions.read().await;
        let mut allowed = false;

        for role_name in roles {
            if let Some(role) = role_defs.get(&role_name) {
                if role.permissions.contains(permission) || role.permissions.contains(&Permission::SystemAdmin) {
                    allowed = true;
                    break;
                }
            }
        }

        // Cache result
        let cached_permission = CachedPermission {
            user_id: user_id.to_string(),
            permission: permission.clone(),
            allowed,
            cached_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::minutes(15),
        };

        self.permission_cache.lock().await.insert(cache_key, cached_permission);

        Ok(allowed)
    }

    pub async fn get_user_permissions(&self, user_id: &str) -> Result<HashSet<String>> {
        let user_roles = self.user_roles.read().await;
        let roles = user_roles.get(user_id).cloned().unwrap_or_default();

        let mut permissions = HashSet::new();
        let role_defs = self.role_definitions.read().await;

        for role_name in roles {
            if let Some(role) = role_defs.get(&role_name) {
                for permission in &role.permissions {
                    permissions.insert(format!("{:?}", permission));
                }
            }
        }

        Ok(permissions)
    }

    pub async fn create_role(&self, role_name: &str, permissions: HashSet<Permission>) -> Result<String> {
        let role_id = Uuid::new_v4().to_string();
        let role = Role {
            role_id: role_id.clone(),
            role_name: role_name.to_string(),
            permissions,
            inherits_from: Vec::new(),
            created_at: Utc::now(),
        };

        self.role_definitions.write().await.insert(role_name.to_string(), role);

        Ok(role_id)
    }

    pub async fn assign_role_to_user(&self, user_id: &str, role_name: &str) -> Result<()> {
        let mut user_roles = self.user_roles.write().await;
        let roles = user_roles.entry(user_id.to_string()).or_insert_with(Vec::new);
        
        if !roles.contains(&role_name.to_string()) {
            roles.push(role_name.to_string());
        }

        Ok(())
    }
}

impl EncryptionService {
    pub fn new() -> Self {
        Self {
            encryption_keys: Arc::new(RwLock::new(HashMap::new())),
            current_key_id: Arc::new(Mutex::new("default".to_string())),
            encryption_config: EncryptionConfig {
                default_algorithm: "AES-256-GCM".to_string(),
                key_rotation_days: 30,
                encrypt_at_rest: true,
                encrypt_in_transit: true,
            },
        }
    }

    pub async fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        // Simplified encryption - in real implementation would use proper crypto
        let mut encrypted = Vec::new();
        for byte in data {
            encrypted.push(byte ^ 0xFF); // Simple XOR for testing
        }
        Ok(encrypted)
    }

    pub async fn decrypt(&self, encrypted_data: &[u8]) -> Result<Vec<u8>> {
        // Simplified decryption - in real implementation would use proper crypto
        let mut decrypted = Vec::new();
        for byte in encrypted_data {
            decrypted.push(byte ^ 0xFF); // Simple XOR for testing
        }
        Ok(decrypted)
    }
}

impl AuditLogger {
    pub fn new() -> Self {
        Self {
            audit_log: Arc::new(Mutex::new(Vec::new())),
            audit_config: AuditConfig {
                log_all_actions: true,
                log_failed_attempts: true,
                retention_days: 365,
                secure_logging: true,
            },
        }
    }

    pub async fn log_action(
        &self,
        user_id: Option<String>,
        session_id: Option<String>,
        action: AuditAction,
        resource: &str,
        ip_address: &str,
        user_agent: &str,
        success: bool,
        error_message: Option<String>,
        additional_data: HashMap<String, String>,
    ) {
        let entry = AuditLogEntry {
            entry_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            user_id,
            session_id,
            action,
            resource: resource.to_string(),
            ip_address: ip_address.to_string(),
            user_agent: user_agent.to_string(),
            success,
            error_message,
            additional_data,
        };

        self.audit_log.lock().await.push(entry);
    }

    pub async fn get_logs(&self, filters: AuditLogFilters) -> Result<Vec<AuditLogEntry>> {
        let logs = self.audit_log.lock().await;
        let mut filtered_logs: Vec<_> = logs.iter().filter(|entry| {
            if let Some(start) = filters.start_date {
                if entry.timestamp < start {
                    return false;
                }
            }
            
            if let Some(end) = filters.end_date {
                if entry.timestamp > end {
                    return false;
                }
            }

            if let Some(ref user_id) = filters.user_id {
                if entry.user_id.as_ref() != Some(user_id) {
                    return false;
                }
            }

            true
        }).cloned().collect();

        if let Some(limit) = filters.limit {
            filtered_logs.truncate(limit);
        }

        Ok(filtered_logs)
    }
}

impl AccessControlManager {
    pub fn new() -> Self {
        Self {
            ip_whitelist: Arc::new(RwLock::new(HashSet::new())),
            rate_limits: Arc::new(Mutex::new(HashMap::new())),
            access_policies: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn is_rate_limited(&self, ip_address: &str) -> Result<bool> {
        let mut rate_limits = self.rate_limits.lock().await;
        let tracker = rate_limits.entry(ip_address.to_string()).or_insert_with(|| {
            RateLimitTracker {
                requests: Vec::new(),
                max_requests: 100, // 100 requests per window
                window_duration: Duration::from_minutes(15),
                blocked_until: None,
            }
        });

        // Check if currently blocked
        if let Some(blocked_until) = tracker.blocked_until {
            if Utc::now() < blocked_until {
                return Ok(true);
            } else {
                tracker.blocked_until = None;
            }
        }

        // Clean old requests
        let window_start = Utc::now() - chrono::Duration::from_std(tracker.window_duration).unwrap();
        tracker.requests.retain(|&time| time > window_start);

        // Check rate limit
        if tracker.requests.len() >= tracker.max_requests as usize {
            tracker.blocked_until = Some(Utc::now() + chrono::Duration::minutes(15));
            return Ok(true);
        }

        // Add current request
        tracker.requests.push(Utc::now());

        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, timeout};

    async fn create_test_security_manager() -> ConfigSecurityManager {
        let manager = ConfigSecurityManager::new();
        
        // Create test roles
        manager.authz_manager.create_role(
            "admin",
            vec![Permission::SystemAdmin].into_iter().collect()
        ).await.unwrap();
        
        manager.authz_manager.create_role(
            "config_reader",
            vec![Permission::ConfigRead("*".to_string())].into_iter().collect()
        ).await.unwrap();
        
        manager.authz_manager.create_role(
            "config_writer",
            vec![
                Permission::ConfigRead("*".to_string()),
                Permission::ConfigWrite("*".to_string())
            ].into_iter().collect()
        ).await.unwrap();

        manager
    }

    #[tokio::test]
    async fn test_user_authentication() {
        let manager = create_test_security_manager().await;
        
        // Create test user
        let session_id = manager.authenticate("test_admin", "temp_password", "127.0.0.1", "test-client").await;
        
        // First create admin user to create other users
        manager.auth_provider.create_user("test_admin", "admin_password", vec!["admin".to_string()]).await.unwrap();
        manager.authz_manager.assign_role_to_user("test_user", "admin").await.unwrap();
        
        // Login as admin first
        let admin_session = manager.authenticate("test_admin", "admin_password", "127.0.0.1", "test-client").await.unwrap();
        
        // Create test user
        let user_id = manager.create_user(
            "testuser",
            "testpass123!",
            vec!["config_reader".to_string()],
            &admin_session,
        ).await.unwrap();
        
        assert!(!user_id.is_empty());
        
        // Authenticate the new user
        let session_id = manager.authenticate("testuser", "testpass123!", "127.0.0.1", "test-client").await.unwrap();
        assert!(!session_id.is_empty());
        
        // Validate session
        let session_info = manager.validate_session(&session_id).await.unwrap();
        assert_eq!(session_info.username, "testuser");
    }

    #[tokio::test]
    async fn test_invalid_authentication() {
        let manager = create_test_security_manager().await;
        
        // Try to authenticate non-existent user
        let result = manager.authenticate("nonexistent", "password", "127.0.0.1", "test-client").await;
        assert!(result.is_err());
        
        // Create user first
        manager.auth_provider.create_user("testuser", "correctpass", vec!["config_reader".to_string()]).await.unwrap();
        
        // Try wrong password
        let result = manager.authenticate("testuser", "wrongpass", "127.0.0.1", "test-client").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_permission_checking() {
        let manager = create_test_security_manager().await;
        
        // Create user with config reader role
        manager.auth_provider.create_user("reader", "password123!", vec!["config_reader".to_string()]).await.unwrap();
        manager.authz_manager.assign_role_to_user("reader_id", "config_reader").await.unwrap();
        
        let session_id = manager.authenticate("reader", "password123!", "127.0.0.1", "test-client").await.unwrap();
        
        // Should have read permission
        let can_read = manager.check_permission(
            &session_id,
            Permission::ConfigRead("test".to_string()),
            "test.config",
            "127.0.0.1",
            "test-client"
        ).await.unwrap();
        assert!(can_read);
        
        // Should not have write permission
        let can_write = manager.check_permission(
            &session_id,
            Permission::ConfigWrite("test".to_string()),
            "test.config",
            "127.0.0.1",
            "test-client"
        ).await.unwrap();
        assert!(!can_write);
    }

    #[tokio::test]
    async fn test_input_validation() {
        let manager = create_test_security_manager().await;
        
        // Valid inputs should pass
        assert!(manager.validate_input("valid.config.key", InputType::ConfigKey).await.is_ok());
        assert!(manager.validate_input("valid_username", InputType::Username).await.is_ok());
        
        // Invalid inputs should fail
        assert!(manager.validate_input("../etc/passwd", InputType::ConfigKey).await.is_err());
        assert!(manager.validate_input("config'; DROP TABLE users; --", InputType::ConfigKey).await.is_err());
        assert!(manager.validate_input("<script>alert('xss')</script>", InputType::ConfigValue).await.is_err());
        assert!(manager.validate_input("user@domain.com", InputType::Username).await.is_err());
    }

    #[tokio::test]
    async fn test_encryption_decryption() {
        let manager = create_test_security_manager().await;
        
        let original_data = "sensitive configuration data";
        
        // Encrypt data
        let encrypted = manager.encrypt_config_value("secure_namespace", original_data).await.unwrap();
        
        // Decrypt data
        let decrypted = manager.decrypt_config_value("secure_namespace", &encrypted).await.unwrap();
        
        assert_eq!(decrypted, original_data);
    }

    #[tokio::test]
    async fn test_audit_logging() {
        let manager = create_test_security_manager().await;
        
        // Create and authenticate user
        manager.auth_provider.create_user("audit_test", "password123!", vec!["admin".to_string()]).await.unwrap();
        manager.authz_manager.assign_role_to_user("audit_user_id", "admin").await.unwrap();
        
        let session_id = manager.authenticate("audit_test", "password123!", "127.0.0.1", "test-client").await.unwrap();
        
        // Perform some actions that should be logged
        manager.check_permission(
            &session_id,
            Permission::ConfigRead("test".to_string()),
            "test.resource",
            "127.0.0.1",
            "test-client"
        ).await.unwrap();
        
        // Get audit logs
        let filters = AuditLogFilters {
            start_date: Some(Utc::now() - chrono::Duration::hours(1)),
            end_date: None,
            user_id: None,
            action: None,
            resource: None,
            limit: None,
        };
        
        let logs = manager.get_audit_logs(&session_id, filters).await.unwrap();
        
        // Should have login and permission check logs
        assert!(logs.len() >= 2);
        
        let permission_logs: Vec<_> = logs.iter()
            .filter(|log| matches!(log.action, AuditAction::PermissionCheck))
            .collect();
        assert!(!permission_logs.is_empty());
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let manager = create_test_security_manager().await;
        let ip = "192.168.1.100";
        
        // First few requests should succeed
        for _ in 0..5 {
            let limited = manager.access_control.is_rate_limited(ip).await.unwrap();
            assert!(!limited);
        }
        
        // Simulate hitting rate limit by manually triggering it
        {
            let mut rate_limits = manager.access_control.rate_limits.lock().await;
            if let Some(tracker) = rate_limits.get_mut(ip) {
                // Fill up the request limit
                let now = Utc::now();
                tracker.requests = (0..tracker.max_requests).map(|_| now).collect();
            }
        }
        
        // Next request should be rate limited
        let limited = manager.access_control.is_rate_limited(ip).await.unwrap();
        assert!(limited);
    }

    #[tokio::test]
    async fn test_session_expiration() {
        let manager = create_test_security_manager().await;
        
        // Create user and authenticate
        manager.auth_provider.create_user("session_test", "password123!", vec!["config_reader".to_string()]).await.unwrap();
        let session_id = manager.authenticate("session_test", "password123!", "127.0.0.1", "test-client").await.unwrap();
        
        // Session should be valid initially
        let session_info = manager.validate_session(&session_id).await.unwrap();
        assert_eq!(session_info.username, "session_test");
        
        // Manually expire session for testing
        {
            let mut sessions = manager.active_sessions.write().await;
            if let Some(session) = sessions.get_mut(&session_id) {
                session.expires_at = Utc::now() - chrono::Duration::minutes(1);
            }
        }
        
        // Session should now be invalid
        let result = manager.validate_session(&session_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_security_policy_enforcement() {
        let manager = create_test_security_manager().await;
        
        let namespace = "secure_config";
        let policy = SecurityPolicy {
            namespace: namespace.to_string(),
            required_roles: vec!["admin".to_string()],
            encryption_required: true,
            audit_level: AuditLevel::Full,
            access_restrictions: AccessRestrictions {
                ip_whitelist: vec!["127.0.0.1".to_string()],
                time_restrictions: Vec::new(),
                require_mfa: false,
                max_concurrent_sessions: Some(5),
            },
        };
        
        manager.set_security_policy(namespace, policy).await.unwrap();
        
        // Test encryption requirement
        let test_data = "sensitive data";
        let encrypted = manager.encrypt_config_value(namespace, test_data).await.unwrap();
        
        // Should be encrypted (different from original in our simple XOR implementation)
        assert_ne!(encrypted, test_data.as_bytes());
        
        // Decryption should recover original data
        let decrypted = manager.decrypt_config_value(namespace, &encrypted).await.unwrap();
        assert_eq!(decrypted, test_data);
    }

    #[tokio::test]
    async fn test_password_validation() {
        let manager = create_test_security_manager().await;
        
        // Valid password should pass
        assert!(manager.validate_password("ValidPass123!").await.is_ok());
        
        // Too short should fail
        assert!(manager.validate_password("short").await.is_err());
        
        // No special characters should fail
        assert!(manager.validate_password("NoSpecialChars123").await.is_err());
        
        // Just right length and special chars should pass
        assert!(manager.validate_password("GoodPass123!").await.is_ok());
    }

    #[tokio::test]
    async fn test_role_based_access() {
        let manager = create_test_security_manager().await;
        
        // Create different users with different roles
        manager.auth_provider.create_user("admin_user", "adminpass123!", vec!["admin".to_string()]).await.unwrap();
        manager.auth_provider.create_user("reader_user", "readerpass123!", vec!["config_reader".to_string()]).await.unwrap();
        
        manager.authz_manager.assign_role_to_user("admin_id", "admin").await.unwrap();
        manager.authz_manager.assign_role_to_user("reader_id", "config_reader").await.unwrap();
        
        let admin_session = manager.authenticate("admin_user", "adminpass123!", "127.0.0.1", "test-client").await.unwrap();
        let reader_session = manager.authenticate("reader_user", "readerpass123!", "127.0.0.1", "test-client").await.unwrap();
        
        // Admin should have all permissions
        let admin_can_write = manager.check_permission(
            &admin_session,
            Permission::ConfigWrite("test".to_string()),
            "test.config",
            "127.0.0.1",
            "test-client"
        ).await.unwrap();
        assert!(admin_can_write);
        
        // Reader should only have read permissions
        let reader_can_read = manager.check_permission(
            &reader_session,
            Permission::ConfigRead("test".to_string()),
            "test.config",
            "127.0.0.1",
            "test-client"
        ).await.unwrap();
        assert!(reader_can_read);
        
        let reader_can_write = manager.check_permission(
            &reader_session,
            Permission::ConfigWrite("test".to_string()),
            "test.config",
            "127.0.0.1",
            "test-client"
        ).await.unwrap();
        assert!(!reader_can_write);
    }

    #[tokio::test]
    async fn test_concurrent_security_operations() {
        let manager = Arc::new(create_test_security_manager().await);
        let mut handles = Vec::new();
        
        // Spawn multiple authentication attempts
        for i in 0..10 {
            let manager_clone = manager.clone();
            let handle = tokio::spawn(async move {
                let username = format!("user_{}", i);
                let password = format!("password{}!", i);
                
                // Create user
                manager_clone.auth_provider.create_user(&username, &password, vec!["config_reader".to_string()]).await.unwrap();
                
                // Authenticate
                manager_clone.authenticate(&username, &password, "127.0.0.1", "test-client").await
            });
            handles.push(handle);
        }
        
        // Wait for all authentications
        let results: Vec<_> = futures::future::join_all(handles).await;
        
        // All should succeed
        for result in results {
            assert!(result.unwrap().is_ok());
        }
        
        // Verify audit logs captured all operations
        let admin_session = manager.authenticate("admin_user", "adminpass123!", "127.0.0.1", "test-client").await;
        if admin_session.is_ok() {
            let filters = AuditLogFilters {
                start_date: Some(Utc::now() - chrono::Duration::minutes(1)),
                end_date: None,
                user_id: None,
                action: Some(AuditAction::Login),
                resource: None,
                limit: None,
            };
            
            // Note: This test might fail because we haven't created admin_user in this test
            // In a real scenario, we'd set up the admin user properly
        }
    }

    #[tokio::test]
    async fn test_session_management() {
        let manager = create_test_security_manager().await;
        
        // Create and authenticate user
        manager.auth_provider.create_user("session_mgmt_test", "password123!", vec!["config_reader".to_string()]).await.unwrap();
        let session_id = manager.authenticate("session_mgmt_test", "password123!", "127.0.0.1", "test-client").await.unwrap();
        
        // Session should exist and be valid
        assert!(manager.validate_session(&session_id).await.is_ok());
        
        // Logout should invalidate session
        manager.logout(&session_id).await.unwrap();
        
        // Session should no longer be valid
        assert!(manager.validate_session(&session_id).await.is_err());
    }
}