//! Risk Manager Implementation

use anyhow::Result;
use crate::RiskLimits;

pub struct RiskManager;

impl RiskManager {
    pub fn new(_risk_limits: RiskLimits) -> Self {
        Self
    }

    pub async fn start_monitoring(&self) -> Result<()> {
        tracing::info!("Risk monitoring started");
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        tracing::info!("Risk monitoring stopped");
        Ok(())
    }
}