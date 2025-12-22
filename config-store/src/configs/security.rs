//! Security configuration module
//!
//! Handles security, authentication, and encryption configuration.

use serde::{Deserialize, Serialize};

/// Security configuration for production deployment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    #[serde(default = "default_enable_tls")]
    pub enable_tls: bool,
    #[serde(default = "default_tls_cert_path")]
    pub tls_cert_path: String,
    #[serde(default = "default_tls_key_path")]
    pub tls_key_path: String,
    #[serde(default = "default_api_key_required")]
    pub api_key_required: bool,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_rate_limit_per_minute")]
    pub rate_limit_per_minute: u32,
    #[serde(default = "default_max_request_size_mb")]
    pub max_request_size_mb: u64,
    #[serde(default = "default_enable_cors")]
    pub enable_cors: bool,
    #[serde(default = "default_allowed_origins")]
    pub allowed_origins: Vec<String>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_tls: default_enable_tls(),
            tls_cert_path: default_tls_cert_path(),
            tls_key_path: default_tls_key_path(),
            api_key_required: default_api_key_required(),
            api_key: None,
            rate_limit_per_minute: default_rate_limit_per_minute(),
            max_request_size_mb: default_max_request_size_mb(),
            enable_cors: default_enable_cors(),
            allowed_origins: default_allowed_origins(),
        }
    }
}

/// Circuit breaker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,
    #[serde(default = "default_recovery_timeout_seconds")]
    pub recovery_timeout_seconds: u64,
    #[serde(default = "default_half_open_max_calls")]
    pub half_open_max_calls: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: default_failure_threshold(),
            recovery_timeout_seconds: default_recovery_timeout_seconds(),
            half_open_max_calls: default_half_open_max_calls(),
        }
    }
}

/// Graceful shutdown configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GracefulShutdownConfig {
    #[serde(default = "default_shutdown_timeout_seconds")]
    pub timeout_seconds: u64,
}

impl Default for GracefulShutdownConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: default_shutdown_timeout_seconds(),
        }
    }
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(default = "default_jwt_secret")]
    pub jwt_secret: String,
    #[serde(default = "default_jwt_expiry_hours")]
    pub jwt_expiry_hours: u64,
    #[serde(default = "default_enable_basic_auth")]
    pub enable_basic_auth: bool,
    #[serde(default = "default_enable_oauth")]
    pub enable_oauth: bool,
    #[serde(default)]
    pub oauth_providers: Vec<OAuthProvider>,
}

/// OAuth provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthProvider {
    pub name: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub scope: Vec<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: default_jwt_secret(),
            jwt_expiry_hours: default_jwt_expiry_hours(),
            enable_basic_auth: default_enable_basic_auth(),
            enable_oauth: default_enable_oauth(),
            oauth_providers: Vec::new(),
        }
    }
}

/// Encryption configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    #[serde(default = "default_encryption_algorithm")]
    pub algorithm: String,
    #[serde(default = "default_key_size_bits")]
    pub key_size_bits: u32,
    #[serde(default = "default_enable_at_rest_encryption")]
    pub enable_at_rest_encryption: bool,
    #[serde(default = "default_enable_in_transit_encryption")]
    pub enable_in_transit_encryption: bool,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            algorithm: default_encryption_algorithm(),
            key_size_bits: default_key_size_bits(),
            enable_at_rest_encryption: default_enable_at_rest_encryption(),
            enable_in_transit_encryption: default_enable_in_transit_encryption(),
        }
    }
}

// Default value functions
fn default_enable_tls() -> bool {
    false
}
fn default_tls_cert_path() -> String {
    "certs/server.crt".to_string()
}
fn default_tls_key_path() -> String {
    "certs/server.key".to_string()
}
fn default_api_key_required() -> bool {
    false
}
fn default_rate_limit_per_minute() -> u32 {
    100
}
fn default_max_request_size_mb() -> u64 {
    10
}
fn default_enable_cors() -> bool {
    true
}
fn default_allowed_origins() -> Vec<String> {
    vec!["*".to_string()]
}
fn default_failure_threshold() -> u32 {
    5
}
fn default_recovery_timeout_seconds() -> u64 {
    60
}
fn default_half_open_max_calls() -> u32 {
    3
}
fn default_shutdown_timeout_seconds() -> u64 {
    30
}
fn default_jwt_secret() -> String {
    "your-secret-key".to_string()
}
fn default_jwt_expiry_hours() -> u64 {
    24
}
fn default_enable_basic_auth() -> bool {
    false
}
fn default_enable_oauth() -> bool {
    false
}
fn default_encryption_algorithm() -> String {
    "AES-256-GCM".to_string()
}
fn default_key_size_bits() -> u32 {
    256
}
fn default_enable_at_rest_encryption() -> bool {
    false
}
fn default_enable_in_transit_encryption() -> bool {
    true
}
