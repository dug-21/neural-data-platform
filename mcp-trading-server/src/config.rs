use crate::error::{Error, Result};
use serde::Deserialize;
use std::env;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub neural_service_url: String,
    pub agent_service_url: String,
    pub server_port: u16,
    pub log_level: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgresql://postgres:postgres@localhost/neural_trader".to_string()
            }),
            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379/0".to_string()),
            neural_service_url: env::var("NEURAL_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:8001".to_string()),
            agent_service_url: env::var("AGENT_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:8002".to_string()),
            server_port: env::var("MCP_SERVER_PORT")
                .unwrap_or_else(|_| "8003".to_string())
                .parse()
                .map_err(|e| Error::Config(format!("Invalid port: {}", e)))?,
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
        })
    }
}
