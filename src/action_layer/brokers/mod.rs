//! Broker integrations for the Action Layer
//!
//! This module provides broker integrations starting with Alpaca for MVP.

use crate::action_layer::{ActionLayerError, BrokerConfig, BrokerInterface};
use std::sync::Arc;

pub mod alpaca;
pub mod paper_trading;

/// Create a broker instance based on configuration
pub async fn create_broker(config: &BrokerConfig) -> Result<Arc<dyn BrokerInterface>, ActionLayerError> {
    match config.name.as_str() {
        "alpaca" => {
            if config.paper_trading {
                Ok(Arc::new(alpaca::AlpacaPaperBroker::new(config).await?))
            } else {
                Ok(Arc::new(alpaca::AlpacaLiveBroker::new(config).await?))
            }
        }
        "paper" => {
            Ok(Arc::new(paper_trading::PaperTradingBroker::new(config).await?))
        }
        _ => Err(ActionLayerError::Broker(format!("Unknown broker: {}", config.name)))
    }
}