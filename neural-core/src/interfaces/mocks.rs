use async_trait::async_trait;
use mockall::mock;
use crate::interfaces::grpc_traits::*;

// Mock implementations for service traits
mod mock_implementations {

    use super::*;

    mock! {
        pub MarketDataService {}
        
        #[async_trait]
        impl MarketDataServiceTrait for MarketDataService {
            async fn get_health_status(&self) -> Result<String, String>;
        }
    }

    mock! {
        pub FeatureEngineeringService {}
        
        #[async_trait]
        impl FeatureEngineeringServiceTrait for FeatureEngineeringService {
            async fn get_health_status(&self) -> Result<String, String>;
        }
    }

    mock! {
        pub ModelManagementService {}
        
        #[async_trait]
        impl ModelManagementServiceTrait for ModelManagementService {
            async fn get_health_status(&self) -> Result<String, String>;
        }
    }

    mock! {
        pub TradingService {}
        
        #[async_trait]
        impl TradingServiceTrait for TradingService {
            async fn get_health_status(&self) -> Result<String, String>;
        }
    }

    // Simple test implementations
    impl MockMarketDataService {
        pub fn expect_healthy() -> Self {
            let mut mock = Self::new();
            mock.expect_get_health_status()
                .returning(|| Ok("healthy".to_string()));
            mock
        }
    }

    impl MockFeatureEngineeringService {
        pub fn expect_healthy() -> Self {
            let mut mock = Self::new();
            mock.expect_get_health_status()
                .returning(|| Ok("healthy".to_string()));
            mock
        }
    }

    impl MockModelManagementService {
        pub fn expect_healthy() -> Self {
            let mut mock = Self::new();
            mock.expect_get_health_status()
                .returning(|| Ok("healthy".to_string()));
            mock
        }
    }

    impl MockTradingService {
        pub fn expect_healthy() -> Self {
            let mut mock = Self::new();
            mock.expect_get_health_status()
                .returning(|| Ok("healthy".to_string()));
            mock
        }
    }
}

// Use the mock implementations
pub use mock_implementations::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_market_data_mock() {
        let mock = MockMarketDataService::expect_healthy();
        let result = mock.get_health_status().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "healthy");
    }
    
    #[tokio::test]
    async fn test_feature_engineering_mock() {
        let mock = MockFeatureEngineeringService::expect_healthy();
        let result = mock.get_health_status().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "healthy");
    }

    #[tokio::test]
    async fn test_model_management_mock() {
        let mock = MockModelManagementService::expect_healthy();
        let result = mock.get_health_status().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "healthy");
    }

    #[tokio::test]
    async fn test_trading_mock() {
        let mock = MockTradingService::expect_healthy();
        let result = mock.get_health_status().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "healthy");
    }
}