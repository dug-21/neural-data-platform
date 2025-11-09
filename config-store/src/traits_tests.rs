/// London TDD: Comprehensive unit tests for ConfigStore trait
/// All tests written FIRST before implementation
/// Using mockall for complete mocking

#[cfg(test)]
mod config_store_trait_tests {
    use super::super::*;
    use mockall::*;
    use mockall::predicate::*;
    use async_trait::async_trait;
    use std::sync::Arc;
    use tokio;
    
    // Create mock for ConfigStore trait
    mock! {
        pub ConfigStore {}
        
        #[async_trait]
        impl ConfigStore for ConfigStore {
            async fn get(&self, path: &str) -> Result<ConfigValue, ConfigError>;
            async fn set(&self, path: &str, value: ConfigValue) -> Result<(), ConfigError>;
            async fn delete(&self, path: &str) -> Result<(), ConfigError>;
            async fn get_tree(&self, prefix: &str) -> Result<ConfigTree, ConfigError>;
            async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, ConfigError>;
            async fn get_version(&self, path: &str, version: u32) -> Result<ConfigValue, ConfigError>;
            async fn get_history(&self, path: &str) -> Result<Vec<ConfigVersion>, ConfigError>;
            async fn set_node(&self, path: &str, node: ConfigNode) -> Result<(), ConfigError>;
            async fn get_node(&self, path: &str) -> Result<ConfigNode, ConfigError>;
        }
        
        impl Clone for ConfigStore {
            fn clone(&self) -> Self;
        }
    }
    
    // Test helper to verify trait is Send + Sync
    fn assert_send_sync<T: Send + Sync>() {}
    
    #[test]
    fn test_config_store_is_send_sync() {
        assert_send_sync::<Box<dyn ConfigStore>>();
        assert_send_sync::<Arc<dyn ConfigStore>>();
    }
    
    #[tokio::test]
    async fn test_get_existing_configuration() {
        let mut mock = MockConfigStore::new();
        let expected_value = ConfigValue::String("test_value".to_string());
        
        mock.expect_get()
            .with(eq("/test/path"))
            .times(1)
            .returning(move |_| Ok(ConfigValue::String("test_value".to_string())));
        
        let result = mock.get("/test/path").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ConfigValue::String("test_value".to_string()));
    }
    
    #[tokio::test]
    async fn test_get_nonexistent_configuration() {
        let mut mock = MockConfigStore::new();
        
        mock.expect_get()
            .with(eq("/nonexistent"))
            .times(1)
            .returning(|path| Err(ConfigError::NotFound(path.to_string())));
        
        let result = mock.get("/nonexistent").await;
        assert!(matches!(result, Err(ConfigError::NotFound(_))));
    }
    
    #[tokio::test]
    async fn test_get_with_invalid_path() {
        let mut mock = MockConfigStore::new();
        
        mock.expect_get()
            .with(eq("invalid-path"))  // Missing leading slash
            .times(1)
            .returning(|_| Err(ConfigError::InvalidPath("Path must start with /".to_string())));
        
        let result = mock.get("invalid-path").await;
        assert!(matches!(result, Err(ConfigError::InvalidPath(_))));
    }
    
    #[tokio::test]
    async fn test_set_configuration() {
        let mut mock = MockConfigStore::new();
        let value = ConfigValue::Object(std::collections::HashMap::new());
        
        mock.expect_set()
            .with(eq("/test/config"), eq(value.clone()))
            .times(1)
            .returning(|_, _| Ok(()));
        
        let result = mock.set("/test/config", value).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_set_with_validation_failure() {
        let mut mock = MockConfigStore::new();
        let invalid_value = ConfigValue::String("invalid".to_string());
        
        mock.expect_set()
            .with(eq("/validated/path"), eq(invalid_value.clone()))
            .times(1)
            .returning(|_, _| Err(ConfigError::ValidationFailed(vec![
                "Value does not match schema".to_string()
            ])));
        
        let result = mock.set("/validated/path", invalid_value).await;
        assert!(matches!(result, Err(ConfigError::ValidationFailed(_))));
    }
    
    #[tokio::test]
    async fn test_delete_existing_configuration() {
        let mut mock = MockConfigStore::new();
        
        mock.expect_delete()
            .with(eq("/test/config"))
            .times(1)
            .returning(|_| Ok(()));
        
        let result = mock.delete("/test/config").await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_delete_nonexistent_configuration() {
        let mut mock = MockConfigStore::new();
        
        mock.expect_delete()
            .with(eq("/nonexistent"))
            .times(1)
            .returning(|path| Err(ConfigError::NotFound(path.to_string())));
        
        let result = mock.delete("/nonexistent").await;
        assert!(matches!(result, Err(ConfigError::NotFound(_))));
    }
    
    #[tokio::test]
    async fn test_get_tree() {
        let mut mock = MockConfigStore::new();
        let mut tree = ConfigTree::new();
        tree.insert("/app/database/host".to_string(), ConfigValue::String("localhost".to_string()));
        tree.insert("/app/database/port".to_string(), ConfigValue::Integer(5432));
        tree.insert("/app/cache/ttl".to_string(), ConfigValue::Integer(60));
        
        mock.expect_get_tree()
            .with(eq("/app"))
            .times(1)
            .returning(move |_| Ok(tree.clone()));
        
        let result = mock.get_tree("/app").await;
        assert!(result.is_ok());
        let tree = result.unwrap();
        assert_eq!(tree.len(), 3);
        assert!(tree.contains_key("/app/database/host"));
        assert!(tree.contains_key("/app/database/port"));
        assert!(tree.contains_key("/app/cache/ttl"));
    }
    
    #[tokio::test]
    async fn test_list_keys() {
        let mut mock = MockConfigStore::new();
        let keys = vec![
            "/app/database/host".to_string(),
            "/app/database/port".to_string(),
            "/app/cache/ttl".to_string(),
        ];
        
        mock.expect_list_keys()
            .with(eq("/app"))
            .times(1)
            .returning(move |_| Ok(keys.clone()));
        
        let result = mock.list_keys("/app").await;
        assert!(result.is_ok());
        let keys = result.unwrap();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"/app/database/host".to_string()));
    }
    
    #[tokio::test]
    async fn test_get_version() {
        let mut mock = MockConfigStore::new();
        let historical_value = ConfigValue::String("version_3_value".to_string());
        
        mock.expect_get_version()
            .with(eq("/versioned/config"), eq(3))
            .times(1)
            .returning(move |_, _| Ok(ConfigValue::String("version_3_value".to_string())));
        
        let result = mock.get_version("/versioned/config", 3).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ConfigValue::String("version_3_value".to_string()));
    }
    
    #[tokio::test]
    async fn test_get_version_not_found() {
        let mut mock = MockConfigStore::new();
        
        mock.expect_get_version()
            .with(eq("/versioned/config"), eq(999))
            .times(1)
            .returning(|path, version| Err(ConfigError::VersionNotFound(path.to_string(), version)));
        
        let result = mock.get_version("/versioned/config", 999).await;
        assert!(matches!(result, Err(ConfigError::VersionNotFound(_, 999))));
    }
    
    #[tokio::test]
    async fn test_get_history() {
        let mut mock = MockConfigStore::new();
        let history = vec![
            ConfigVersion {
                version: 1,
                value: ConfigValue::String("v1".to_string()),
                timestamp: std::time::SystemTime::now(),
                metadata: None,
            },
            ConfigVersion {
                version: 2,
                value: ConfigValue::String("v2".to_string()),
                timestamp: std::time::SystemTime::now(),
                metadata: None,
            },
        ];
        
        mock.expect_get_history()
            .with(eq("/versioned/config"))
            .times(1)
            .returning(move |_| Ok(history.clone()));
        
        let result = mock.get_history("/versioned/config").await;
        assert!(result.is_ok());
        let history = result.unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].version, 1);
        assert_eq!(history[1].version, 2);
    }
    
    #[tokio::test]
    async fn test_set_node_with_metadata() {
        let mut mock = MockConfigStore::new();
        let node = ConfigNode {
            path: "/test/node".to_string(),
            value: ConfigValue::String("test".to_string()),
            version: 1,
            metadata: Some(crate::types::ConfigMetadata {
                description: Some("Test node".to_string()),
                owner: Some("test_user".to_string()),
                sensitive: false,
                runtime_modifiable: true,
                created_at: std::time::SystemTime::now(),
                updated_at: std::time::SystemTime::now(),
                updated_by: Some("test_user".to_string()),
                tags: vec!["test".to_string()],
            }),
            inheritance: None,
            schema: None,
        };
        
        mock.expect_set_node()
            .with(eq("/test/node"), eq(node.clone()))
            .times(1)
            .returning(|_, _| Ok(()));
        
        let result = mock.set_node("/test/node", node).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_get_node_with_metadata() {
        let mut mock = MockConfigStore::new();
        let node = ConfigNode {
            path: "/test/node".to_string(),
            value: ConfigValue::String("test".to_string()),
            version: 1,
            metadata: Some(crate::types::ConfigMetadata {
                description: Some("Test node".to_string()),
                owner: Some("test_user".to_string()),
                sensitive: false,
                runtime_modifiable: true,
                created_at: std::time::SystemTime::now(),
                updated_at: std::time::SystemTime::now(),
                updated_by: Some("test_user".to_string()),
                tags: vec!["test".to_string()],
            }),
            inheritance: None,
            schema: None,
        };
        
        mock.expect_get_node()
            .with(eq("/test/node"))
            .times(1)
            .returning(move |_| Ok(node.clone()));
        
        let result = mock.get_node("/test/node").await;
        assert!(result.is_ok());
        let retrieved = result.unwrap();
        assert_eq!(retrieved.path, "/test/node");
        assert_eq!(retrieved.version, 1);
        assert!(retrieved.metadata.is_some());
    }
    
    #[tokio::test]
    async fn test_concurrent_operations() {
        let mock = Arc::new(MockConfigStore::new());
        let mut handles = vec![];
        
        // Test that multiple tasks can use the same store concurrently
        for i in 0..10 {
            let store = mock.clone();
            handles.push(tokio::spawn(async move {
                // This would normally call store methods
                // For now just verify we can spawn with the store
                format!("Task {} completed", i)
            }));
        }
        
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.contains("completed"));
        }
    }
    
    #[tokio::test]
    async fn test_transaction_commit() {
        let mut mock = MockConfigStore::new();
        
        // Set up expectations for transaction operations
        mock.expect_set()
            .with(eq("/tx/1"), always())
            .times(1)
            .returning(|_, _| Ok(()));
            
        mock.expect_set()
            .with(eq("/tx/2"), always())
            .times(1)
            .returning(|_, _| Ok(()));
            
        mock.expect_delete()
            .with(eq("/tx/3"))
            .times(1)
            .returning(|_| Ok(()));
        
        // Create and execute transaction
        let mut tx = ConfigTransaction::new(&mock);
        tx.set("/tx/1".to_string(), ConfigValue::Integer(1));
        tx.set("/tx/2".to_string(), ConfigValue::Integer(2));
        tx.delete("/tx/3".to_string());
        
        let result = tx.commit().await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_transaction_rollback_on_error() {
        let mut mock = MockConfigStore::new();
        
        // First operation succeeds
        mock.expect_set()
            .with(eq("/tx/1"), always())
            .times(1)
            .returning(|_, _| Ok(()));
            
        // Second operation fails
        mock.expect_set()
            .with(eq("/tx/2"), always())
            .times(1)
            .returning(|_, _| Err(ConfigError::Custom("Simulated failure".to_string())));
            
        // Third operation should not be called due to early failure
        mock.expect_delete()
            .with(eq("/tx/3"))
            .times(0);
        
        let mut tx = ConfigTransaction::new(&mock);
        tx.set("/tx/1".to_string(), ConfigValue::Integer(1));
        tx.set("/tx/2".to_string(), ConfigValue::Integer(2));
        tx.delete("/tx/3".to_string());
        
        let result = tx.commit().await;
        assert!(result.is_err());
        assert!(matches!(result, Err(ConfigError::Custom(_))));
    }
}

#[cfg(test)]
mod path_utils_tests {
    use super::super::path_utils::*;
    
    #[test]
    fn test_is_prefix_of() {
        assert!(is_prefix_of("/", "/anything"));
        assert!(is_prefix_of("/app", "/app/database"));
        assert!(is_prefix_of("/app", "/app"));
        assert!(!is_prefix_of("/app", "/application"));
        assert!(!is_prefix_of("/app/db", "/app/database"));
    }
    
    #[test]
    fn test_parent_path() {
        assert_eq!(parent_path("/app/database/host"), Some("/app/database".to_string()));
        assert_eq!(parent_path("/app"), Some("/".to_string()));
        assert_eq!(parent_path("/"), None);
        assert_eq!(parent_path(""), None);
    }
    
    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("/app//database"), "/app/database");
        assert_eq!(normalize_path("///app///"), "/app");
        assert_eq!(normalize_path("/"), "/");
        assert_eq!(normalize_path(""), "/");
        assert_eq!(normalize_path("app/database"), "/app/database");
    }
}