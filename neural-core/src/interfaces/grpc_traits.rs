use async_trait::async_trait;

// TODO: Full gRPC traits will be enabled when proto generation is working
// Always provide stub traits for now (until proto generation works)
mod grpc_disabled {
    use super::*;

    /// Stub trait for Market Data Service when gRPC is disabled
    #[async_trait]
    pub trait MarketDataServiceTrait: Send + Sync + 'static {
        async fn get_health_status(&self) -> Result<String, String>;
    }

    /// Stub trait for Feature Engineering Service when gRPC is disabled
    #[async_trait]
    pub trait FeatureEngineeringServiceTrait: Send + Sync + 'static {
        async fn get_health_status(&self) -> Result<String, String>;
    }

    /// Stub trait for Model Management Service when gRPC is disabled
    #[async_trait]
    pub trait ModelManagementServiceTrait: Send + Sync + 'static {
        async fn get_health_status(&self) -> Result<String, String>;
    }

    /// Stub trait for Trading Service when gRPC is disabled
    #[async_trait]
    pub trait TradingServiceTrait: Send + Sync + 'static {
        async fn get_health_status(&self) -> Result<String, String>;
    }
}

// Always use stub traits for now (until proto generation works)
pub use grpc_disabled::*;