//! Event Consumer Implementation

use anyhow::Result;
use std::sync::Arc;
use crate::daa::coordinator::DAACoordinator;

pub struct EventConsumer;

impl EventConsumer {
    pub async fn new(
        _redis_url: String,
        _daa_coordinator: Arc<DAACoordinator>,
    ) -> Result<Self> {
        Ok(Self)
    }

    pub async fn start(&self) -> Result<()> {
        tracing::info!("Event Consumer started");
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        tracing::info!("Event Consumer stopped");
        Ok(())
    }
}